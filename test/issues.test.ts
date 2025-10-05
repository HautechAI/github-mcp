import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import nock from 'nock';
import { listIssues, getIssue, listIssueCommentsPlain } from '../src/handlers/issues.js';

const API = 'https://api.github.com';

beforeEach(() => {
  process.env.GITHUB_TOKEN = 't';
  delete process.env.GITHUB_BASE_URL;
});

afterEach(() => {
  nock.cleanAll();
});

describe('issues', () => {
  it('listIssues maps minimal fields and pagination', async () => {
    const scope = nock(API)
      .post('/graphql')
      .reply(200, {
        data: {
          rateLimit: { remaining: 4999, used: 1, resetAt: new Date().toISOString() },
          repository: {
            issues: {
              nodes: [
                { id: 'I_1', number: 1, title: 'a', state: 'OPEN', createdAt: '2020-01-01T00:00:00Z', updatedAt: '2020-01-02T00:00:00Z', author: { login: 'alice' } },
              ],
              pageInfo: { hasNextPage: true, endCursor: 'c1' },
            },
          },
        },
      });
    const res: any = await listIssues({ owner: 'o', repo: 'r', include_author: true, limit: 1 });
    expect(res.items[0].author_login).toBe('alice');
    expect(res.meta.next_cursor).toBe('c1');
    scope.done();
  });

  it('getIssue returns 404 mapped', async () => {
    const scope = nock(API)
      .post('/graphql')
      .reply(200, { data: { rateLimit: { remaining: 4999, used: 1 }, repository: { issue: null } } });
    const res: any = await getIssue({ owner: 'o', repo: 'r', number: 999 });
    expect('error' in res).toBe(true);
    scope.done();
  });

  it('listIssueCommentsPlain returns comments', async () => {
    const scope = nock(API)
      .post('/graphql')
      .reply(200, {
        data: {
          rateLimit: { remaining: 4999, used: 2 },
          repository: {
            issue: {
              comments: {
                nodes: [{ id: 'C_1', body: 'hi', createdAt: '2020-01-01T00:00:00Z', updatedAt: '2020-01-01T00:00:00Z', author: { login: 'bob' } }],
                pageInfo: { hasNextPage: false, endCursor: null },
              },
            },
          },
        },
      });
    const res: any = await listIssueCommentsPlain({ owner: 'o', repo: 'r', number: 1, include_author: true });
    expect(res.items[0].author_login).toBe('bob');
    scope.done();
  });
});
