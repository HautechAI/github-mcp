import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
vi.mock('../src/clients/graphql.js', () => {
    return {
        createGraphqlClient: () => async (_q, _v) => {
            if (_q.includes('ListIssues')) {
                return {
                    rateLimit: { remaining: 4999, used: 1, resetAt: new Date().toISOString() },
                    repository: {
                        issues: {
                            nodes: [
                                { id: 'I_1', number: 1, title: 'a', state: 'OPEN', createdAt: '2020-01-01T00:00:00Z', updatedAt: '2020-01-02T00:00:00Z', author: { login: 'alice' } },
                            ],
                            pageInfo: { hasNextPage: true, endCursor: 'c1' },
                        },
                    },
                };
            }
            if (_q.includes('GetIssue')) {
                return { data: { rateLimit: { remaining: 4999, used: 1 }, repository: { issue: null } } };
            }
            if (_q.includes('ListIssueComments')) {
                return {
                    rateLimit: { remaining: 4999, used: 2 },
                    repository: {
                        issue: {
                            comments: {
                                nodes: [
                                    { id: 'C_1', body: 'hi', createdAt: '2020-01-01T00:00:00Z', updatedAt: '2020-01-01T00:00:00Z', author: { login: 'bob' } },
                                ],
                                pageInfo: { hasNextPage: false, endCursor: null },
                            },
                        },
                    },
                };
            }
            return {};
        },
    };
});
import { listIssues, getIssue, listIssueCommentsPlain } from '../src/handlers/issues.js';
beforeEach(() => {
    process.env.GITHUB_TOKEN = 't';
    delete process.env.GITHUB_BASE_URL;
});
afterEach(() => {
    vi.clearAllMocks();
});
describe('issues', () => {
    it('listIssues maps minimal fields and pagination', async () => {
        const res = await listIssues({ owner: 'o', repo: 'r', include_author: true, limit: 1 });
        expect(res.items[0].author_login).toBe('alice');
        expect(res.meta.next_cursor).toBe('c1');
    });
    it('getIssue returns 404 mapped', async () => {
        const res = await getIssue({ owner: 'o', repo: 'r', number: 999 });
        expect('error' in res).toBe(true);
    });
    it('listIssueCommentsPlain returns comments', async () => {
        const res = await listIssueCommentsPlain({ owner: 'o', repo: 'r', number: 1, include_author: true });
        expect(res.items[0].author_login).toBe('bob');
    });
});
//# sourceMappingURL=issues.test.js.map