import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import nock from 'nock';
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

const API = 'https://api.github.com';

beforeEach(() => {
  process.env.GITHUB_TOKEN = 't';
  delete process.env.GITHUB_BASE_URL;
});

afterEach(() => {
  nock.cleanAll();
});

describe('workflows', () => {
  it('listWorkflowsLight maps fields', async () => {
    nock(API)
      .get('/repos/o/r/actions/workflows')
      .query(true)
      .reply(200, { workflows: [{ id: 1, name: 'CI', path: '.github/workflows/ci.yml', state: 'active' }] }, {
        'X-RateLimit-Remaining': '4999',
        'X-RateLimit-Used': '1',
      });
    const r: any = await listWorkflowsLight({ owner: 'o', repo: 'r' });
    expect(r.items[0].name).toBe('CI');
  });

  it('listWorkflowRunsLight maps runs', async () => {
    nock(API)
      .get('/repos/o/r/actions/workflows/1/runs')
      .query(true)
      .reply(200, { workflow_runs: [{ id: 2, run_number: 5, event: 'push', status: 'completed', conclusion: 'success', head_sha: 'abc', created_at: '2020', updated_at: '2020' }] }, {
        'X-RateLimit-Remaining': '4999',
        'X-RateLimit-Used': '1',
      });
    const r: any = await listWorkflowRunsLight({ owner: 'o', repo: 'r', workflow_id: 1 });
    expect(r.items[0].id).toBe(2);
  });

  it('getWorkflowRunLight maps item', async () => {
    nock(API)
      .get('/repos/o/r/actions/runs/2')
      .query(true)
      .reply(200, { id: 2, run_number: 5, event: 'push', status: 'completed', conclusion: 'success', head_sha: 'abc', created_at: '2020', updated_at: '2020' }, {
        'X-RateLimit-Remaining': '4999',
        'X-RateLimit-Used': '1',
      });
    const r: any = await getWorkflowRunLight({ owner: 'o', repo: 'r', run_id: 2 });
    expect(r.item.id).toBe(2);
  });

  it('listWorkflowJobsLight maps items', async () => {
    nock(API)
      .get('/repos/o/r/actions/runs/2/jobs')
      .query(true)
      .reply(200, { jobs: [{ id: 10, name: 'test', status: 'completed', conclusion: 'success', started_at: '2020', completed_at: '2020' }] }, {
        'X-RateLimit-Remaining': '4999',
        'X-RateLimit-Used': '1',
      });
    const r: any = await listWorkflowJobsLight({ owner: 'o', repo: 'r', run_id: 2 });
    expect(r.items[0].name).toBe('test');
  });

  it('getWorkflowJobLogs requires redirect and tails', async () => {
    nock(API)
      .get('/repos/o/r/actions/jobs/11/logs')
      .reply(302, undefined, { Location: 'https://logs.example.com/logs.zip', 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' });
    const JSZip = (await import('jszip')).default;
    const zip = new JSZip();
    zip.file('a.txt', 'line1\nline2\nline3');
    const content = await zip.generateAsync({ type: 'nodebuffer' });
    nock('https://logs.example.com').get('/logs.zip').reply(200, content);
    const r: any = await getWorkflowJobLogs({ owner: 'o', repo: 'r', job_id: 11, tail_lines: 2 });
    expect(r.truncated).toBe(true);
    expect(r.logs.trim().split('\n').length).toBe(2);
  });

  it('rerun/cancel endpoints return ok', async () => {
    nock(API).post('/repos/o/r/actions/runs/2/rerun').reply(201, { id: 3 }, { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' });
    nock(API).post('/repos/o/r/actions/runs/2/rerun-failed-jobs').reply(201, { id: 4 }, { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' });
    nock(API).post('/repos/o/r/actions/runs/2/cancel').reply(202, {}, { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' });
    const a: any = await rerunWorkflowRun({ owner: 'o', repo: 'r', run_id: 2 });
    const b: any = await rerunWorkflowRunFailed({ owner: 'o', repo: 'r', run_id: 2 });
    const c: any = await cancelWorkflowRun({ owner: 'o', repo: 'r', run_id: 2 });
    expect(a.ok && b.ok && c.ok).toBe(true);
  });
});
