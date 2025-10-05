import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

vi.mock('../src/clients/rest.js', () => {
  return {
    createRestClient: () => ({
      async request(route: string, _params: any) {
        if (route === 'GET /repos/{owner}/{repo}/actions/workflows') {
          return { data: { workflows: [{ id: 1, name: 'CI', path: '.github/workflows/ci.yml', state: 'active' }] }, headers: { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' } } as any;
        }
        if (route === 'GET /repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs') {
          return { data: { workflow_runs: [{ id: 2, run_number: 5, event: 'push', status: 'completed', conclusion: 'success', head_sha: 'abc', created_at: '2020', updated_at: '2020' }] }, headers: { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' } } as any;
        }
        if (route === 'GET /repos/{owner}/{repo}/actions/runs/{run_id}') {
          return { data: { id: 2, run_number: 5, event: 'push', status: 'completed', conclusion: 'success', head_sha: 'abc', created_at: '2020', updated_at: '2020' }, headers: { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' } } as any;
        }
        if (route === 'GET /repos/{owner}/{repo}/actions/runs/{run_id}/jobs') {
          return { data: { jobs: [{ id: 10, name: 'test', status: 'completed', conclusion: 'success', started_at: '2020', completed_at: '2020' }] }, headers: { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' } } as any;
        }
        if (route === 'GET /repos/{owner}/{repo}/actions/jobs/{job_id}/logs') {
          // Simulate redirect headers
          return { status: 302, data: Buffer.alloc(0), headers: { location: 'https://logs.example.com/logs.zip', 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' } } as any;
        }
        if (route === 'POST /repos/{owner}/{repo}/actions/runs/{run_id}/rerun') {
          return { status: 201, data: { id: 3 }, headers: { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' } } as any;
        }
        if (route === 'POST /repos/{owner}/{repo}/actions/runs/{run_id}/rerun-failed-jobs') {
          return { status: 201, data: { id: 4 }, headers: { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' } } as any;
        }
        if (route === 'POST /repos/{owner}/{repo}/actions/runs/{run_id}/cancel') {
          return { status: 202, data: {}, headers: { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' } } as any;
        }
        return { data: {}, headers: {} } as any;
      },
    }),
  };
});

vi.mock('../src/utils/http.js', () => {
  return {
    downloadZipAndAggregateText: async () => 'line1\nline2\nline3',
    tailText: (text: string, n: number) => ({ text: text.split('\n').slice(-n).join('\n'), truncated: true }),
    addTimestamps: (t: string) => t,
  };
});

import {
  listWorkflowsLight,
  listWorkflowRunsLight,
  getWorkflowRunLight,
  listWorkflowJobsLight,
  getWorkflowJobLogs,
  rerunWorkflowRun,
  rerunWorkflowRunFailed,
  cancelWorkflowRun,
} from '../src/handlers/workflows.js';

beforeEach(() => {
  process.env.GITHUB_TOKEN = 't';
  delete process.env.GITHUB_BASE_URL;
});

afterEach(() => {
  vi.clearAllMocks();
});

describe('workflows', () => {
  it('listWorkflowsLight maps fields', async () => {
    const r: any = await listWorkflowsLight({ owner: 'o', repo: 'r' });
    expect(r.items[0].name).toBe('CI');
  });

  it('listWorkflowRunsLight maps runs', async () => {
    const r: any = await listWorkflowRunsLight({ owner: 'o', repo: 'r', workflow_id: 1 });
    expect(r.items[0].id).toBe(2);
  });

  it('getWorkflowRunLight maps item', async () => {
    const r: any = await getWorkflowRunLight({ owner: 'o', repo: 'r', run_id: 2 });
    expect(r.item.id).toBe(2);
  });

  it('listWorkflowJobsLight maps items', async () => {
    const r: any = await listWorkflowJobsLight({ owner: 'o', repo: 'r', run_id: 2 });
    expect(r.items[0].name).toBe('test');
  });

  it('getWorkflowJobLogs requires redirect and tails', async () => {
    const r: any = await getWorkflowJobLogs({ owner: 'o', repo: 'r', job_id: 11, tail_lines: 2 });
    expect(r.truncated).toBe(true);
    expect(r.logs.trim().split('\n').length).toBe(2);
  });

  it('rerun/cancel endpoints return ok', async () => {
    const a: any = await rerunWorkflowRun({ owner: 'o', repo: 'r', run_id: 2 });
    const b: any = await rerunWorkflowRunFailed({ owner: 'o', repo: 'r', run_id: 2 });
    const c: any = await cancelWorkflowRun({ owner: 'o', repo: 'r', run_id: 2 });
    expect(a.ok && b.ok && c.ok).toBe(true);
  });
});
