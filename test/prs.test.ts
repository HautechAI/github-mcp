import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

vi.mock('../src/clients/graphql.js', () => {
  return {
    createGraphqlClient: () => async (q: string, _v: any) => {
      if (q.includes('ListPullRequests')) {
        return {
          rateLimit: { remaining: 4999, used: 1 },
          repository: {
            pullRequests: {
              nodes: [{ id: 'P_1', number: 1, title: 't', state: 'OPEN', createdAt: '2020', updatedAt: '2020', author: { login: 'a' } }],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
          },
        };
      }
      if (q.includes('GetPullRequest')) {
        return {
          rateLimit: { remaining: 4999, used: 1 },
          repository: { pullRequest: { id: 'P_1', number: 1, title: 't', body: 'b', state: 'OPEN', isDraft: false, merged: false, mergedAt: null, createdAt: '2020', updatedAt: '2020', author: { login: 'a' } } },
        } as any;
      }
      if (q.includes('GetPrStatusSummary')) {
        return {
          rateLimit: { remaining: 4999, used: 1 },
          repository: {
            pullRequest: {
              commits: {
                nodes: [
                  {
                    commit: {
                      statusCheckRollup: {
                        state: 'FAILURE',
                        contexts: { nodes: [{ __typename: 'CheckRun', name: 'build', conclusion: 'failure' }, { __typename: 'StatusContext', context: 'lint', state: 'SUCCESS' }] },
                      },
                    },
                  },
                ],
              },
            },
          },
        } as any;
      }
      if (q.includes('ListPrComments')) {
        return {
          rateLimit: { remaining: 4999, used: 1 },
          repository: { pullRequest: { comments: { nodes: [{ id: 'C1', body: 'x', createdAt: '2020', updatedAt: '2020', author: { login: 'a' } }], pageInfo: { hasNextPage: false, endCursor: null } } } },
        } as any;
      }
      if (q.includes('ListPrReviewComments')) {
        return {
          rateLimit: { remaining: 4999, used: 1 },
          repository: { pullRequest: { reviewComments: { nodes: [{ id: 'RC_1', body: 'n', createdAt: '2020', updatedAt: '2020', author: { login: 'z' }, path: 'a.ts', line: 10, startLine: 5, side: 'RIGHT', startSide: 'RIGHT', originalLine: 10, originalStartLine: 5, diffHunk: '@@', commit: { oid: 'c' }, originalCommit: { oid: 'oc' }, pullRequestReviewThread: { path: 'a.ts', line: 10, startLine: 5, side: 'RIGHT', startSide: 'RIGHT' } }], pageInfo: { hasNextPage: false, endCursor: null } } } },
        } as any;
      }
      if (q.includes('ListPrReviews')) {
        return {
          rateLimit: { remaining: 4999, used: 1 },
          repository: { pullRequest: { reviews: { nodes: [{ id: 'R_1', state: 'APPROVED', submittedAt: '2020', author: { login: 'rev' } }], pageInfo: { hasNextPage: false, endCursor: null } } } },
        } as any;
      }
      if (q.includes('ListPrCommits')) {
        return {
          rateLimit: { remaining: 4999, used: 1 },
          repository: { pullRequest: { commits: { nodes: [{ commit: { oid: 'sha', messageHeadline: 'm', authoredDate: '2020', author: { user: { login: 'u' } } } }], pageInfo: { hasNextPage: false, endCursor: null } } } },
        } as any;
      }
      return {} as any;
    },
  };
});

vi.mock('../src/clients/rest.js', () => {
  return {
    createRestClient: () => ({
      async request(route: string, params: any) {
        if (route.startsWith('GET /repos/{owner}/{repo}/pulls/{number}/files')) {
          return { data: [{ filename: 'a', status: 'modified', additions: 1, deletions: 0, changes: 1, sha: 's', patch: '---' }], headers: { 'X-RateLimit-Remaining': '4999', 'X-RateLimit-Used': '1' } } as any;
        }
        if (route === 'GET /repos/{owner}/{repo}/pulls/{number}') {
          if ((params.headers?.Accept || '').includes('diff')) return { data: 'diff', headers: {} } as any;
          if ((params.headers?.Accept || '').includes('patch')) return { data: 'patch', headers: {} } as any;
        }
        return { data: {}, headers: {} } as any;
      },
    }),
  };
});

import {
  listPullRequests,
  getPullRequest,
  getPrStatusSummary,
  listPrCommentsPlain,
  listPrReviewCommentsPlain,
  listPrReviewsLight,
  listPrCommitsLight,
  listPrFilesLight,
  getPrDiff,
  getPrPatch,
} from '../src/handlers/prs.js';

beforeEach(() => {
  process.env.GITHUB_TOKEN = 't';
  delete process.env.GITHUB_BASE_URL;
});

afterEach(() => {
  vi.clearAllMocks();
});

describe('prs', () => {
  it('listPullRequests returns items', async () => {
    const r: any = await listPullRequests({ owner: 'o', repo: 'r', include_author: true });
    expect(r.items[0].author_login).toBe('a');
  });

  it('getPullRequest returns minimal fields', async () => {
    const r: any = await getPullRequest({ owner: 'o', repo: 'r', number: 1, include_author: true });
    expect(r.item.author_login).toBe('a');
  });

  it('getPrStatusSummary maps union correctly', async () => {
    const r: any = await getPrStatusSummary({ owner: 'o', repo: 'r', number: 1, include_failing_contexts: true });
    expect(r.item.counts.failure).toBe(1);
    expect(r.item.failing_contexts.length).toBe(1);
  });

  it('listPrFilesLight omits patch by default', async () => {
    const r: any = await listPrFilesLight({ owner: 'o', repo: 'r', number: 1 });
    expect(r.items[0].patch).toBeUndefined();
  });

  it('getPrDiff and getPrPatch use media types', async () => {
    const d: any = await getPrDiff({ owner: 'o', repo: 'r', number: 1 });
    const p: any = await getPrPatch({ owner: 'o', repo: 'r', number: 1 });
    expect(d.diff).toBe('diff');
    expect(p.patch).toBe('patch');
  });

  it('listPrReviewCommentsPlain include_location', async () => {
    const r: any = await listPrReviewCommentsPlain({ owner: 'o', repo: 'r', number: 1, include_location: true });
    expect(r.items[0].path).toBe('a.ts');
  });
});
