import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import nock from 'nock';
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

const API = 'https://api.github.com';

beforeEach(() => {
  process.env.GITHUB_TOKEN = 't';
  delete process.env.GITHUB_BASE_URL;
});

afterEach(() => {
  nock.cleanAll();
});

describe('prs', () => {
  it('listPullRequests returns items', async () => {
    nock(API)
      .post('/graphql')
      .reply(200, {
        data: {
          rateLimit: { remaining: 4999, used: 1 },
          repository: {
            pullRequests: {
              nodes: [{ id: 'P_1', number: 1, title: 't', state: 'OPEN', createdAt: '2020', updatedAt: '2020', author: { login: 'a' } }],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
          },
        },
      });
    const r: any = await listPullRequests({ owner: 'o', repo: 'r', include_author: true });
    expect(r.items[0].author_login).toBe('a');
  });

  it('getPullRequest returns minimal fields', async () => {
    nock(API)
      .post('/graphql')
      .reply(200, {
        data: {
          rateLimit: { remaining: 4999, used: 1 },
          repository: {
            pullRequest: { id: 'P_1', number: 1, title: 't', body: 'b', state: 'OPEN', isDraft: false, merged: false, mergedAt: null, createdAt: '2020', updatedAt: '2020', author: { login: 'a' } },
          },
        },
      });
    const r: any = await getPullRequest({ owner: 'o', repo: 'r', number: 1, include_author: true });
    expect(r.item.author_login).toBe('a');
  });

  it('getPrStatusSummary maps union correctly', async () => {
    nock(API)
      .post('/graphql')
      .reply(200, {
        data: {
          rateLimit: { remaining: 4999, used: 1 },
          repository: {
            pullRequest: {
              commits: {
                nodes: [
                  {
                    commit: {
                      statusCheckRollup: {
                        state: 'FAILURE',
                        contexts: {
                          nodes: [
                            { __typename: 'CheckRun', name: 'build', conclusion: 'failure' },
                            { __typename: 'StatusContext', context: 'lint', state: 'SUCCESS' },
+                          ],
+                        },
+                      },
+                    },
+                  },
+                ],
+              },
+            },
+          },
+        },
+      });
+    const r: any = await getPrStatusSummary({ owner: 'o', repo: 'r', number: 1, include_failing_contexts: true });
+    expect(r.item.counts.failure).toBe(1);
+    expect(r.item.failing_contexts.length).toBe(1);
+  });
+
+  it('listPrFilesLight omits patch by default', async () => {
+    nock(API)
+      .get('/repos/o/r/pulls/1/files')
+      .query(true)
+      .reply(200, [{ filename: 'a', status: 'modified', additions: 1, deletions: 0, changes: 1, sha: 's', patch: '---' }], {
+        'X-RateLimit-Remaining': '4999',
+        'X-RateLimit-Used': '1',
+      });
+    const r: any = await listPrFilesLight({ owner: 'o', repo: 'r', number: 1 });
+    expect(r.items[0].patch).toBeUndefined();
+  });
+
+  it('getPrDiff and getPrPatch use media types', async () => {
+    nock(API).get('/repos/o/r/pulls/1').matchHeader('accept', 'application/vnd.github.v3.diff').reply(200, 'diff');
+    nock(API).get('/repos/o/r/pulls/1').matchHeader('accept', 'application/vnd.github.v3.patch').reply(200, 'patch');
+    const d: any = await getPrDiff({ owner: 'o', repo: 'r', number: 1 });
+    const p: any = await getPrPatch({ owner: 'o', repo: 'r', number: 1 });
+    expect(d.diff).toBe('diff');
+    expect(p.patch).toBe('patch');
+  });
+
+  it('listPrReviewCommentsPlain include_location', async () => {
+    nock(API)
+      .post('/graphql')
+      .reply(200, {
+        data: {
+          rateLimit: { remaining: 4999, used: 1 },
+          repository: {
+            pullRequest: {
+              reviewComments: {
+                nodes: [
+                  {
+                    id: 'RC_1',
+                    body: 'n',
+                    createdAt: '2020',
+                    updatedAt: '2020',
+                    author: { login: 'z' },
+                    path: 'a.ts',
+                    line: 10,
+                    startLine: 5,
+                    side: 'RIGHT',
+                    startSide: 'RIGHT',
+                    originalLine: 10,
+                    originalStartLine: 5,
+                    diffHunk: '@@',
+                    commit: { oid: 'c' },
+                    originalCommit: { oid: 'oc' },
+                    pullRequestReviewThread: { path: 'a.ts', line: 10, startLine: 5, side: 'RIGHT', startSide: 'RIGHT' },
+                  },
+                ],
+                pageInfo: { hasNextPage: false, endCursor: null },
+              },
+            },
+          },
+        },
+      });
+    const r: any = await listPrReviewCommentsPlain({ owner: 'o', repo: 'r', number: 1, include_location: true });
+    expect(r.items[0].path).toBe('a.ts');
+  });
+});
