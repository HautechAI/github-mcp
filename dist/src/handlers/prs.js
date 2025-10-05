import { createGraphqlClient } from '../clients/graphql.js';
import { createRestClient } from '../clients/rest.js';
import { parseGraphqlRate, parseRestRateHeaders } from '../utils/rate.js';
import { restNextCursor } from '../utils/pagination.js';
import { mapError } from '../utils/errors.js';
export async function listPullRequests(input) {
    try {
        const graphql = createGraphqlClient();
        const states = input.state ? [input.state.toUpperCase()] : undefined;
        const query = /* GraphQL */ `
      query ListPullRequests($owner: String!, $repo: String!, $first: Int = 30, $after: String, $states: [PullRequestState!], $base: String, $head: String) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          pullRequests(first: $first, after: $after, states: $states, baseRefName: $base, headRefName: $head, orderBy: { field: UPDATED_AT, direction: DESC }) {
            nodes { id number title state createdAt updatedAt author { login } }
            pageInfo { hasNextPage endCursor }
          }
        }
      }
    `;
        const data = await graphql(query, {
            owner: input.owner,
            repo: input.repo,
            first: input.limit ?? 30,
            after: input.cursor,
            states,
            base: input.base,
            head: input.head,
        });
        const nodes = data.repository?.pullRequests?.nodes || [];
        const pageInfo = data.repository?.pullRequests?.pageInfo || {};
        const items = nodes.map((n) => ({
            id: n.id,
            number: n.number,
            title: n.title,
            state: n.state,
            created_at: n.createdAt,
            updated_at: n.updatedAt,
            ...(input.include_author ? { author_login: n.author?.login || null } : {}),
        }));
        return {
            items,
            meta: { next_cursor: pageInfo.endCursor || null, has_more: Boolean(pageInfo.hasNextPage), rate: parseGraphqlRate(data.rateLimit) },
        };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function getPullRequest(input) {
    try {
        const graphql = createGraphqlClient();
        const query = /* GraphQL */ `
      query GetPullRequest($owner: String!, $repo: String!, $number: Int!) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          pullRequest(number: $number) {
            id number title body state isDraft merged mergedAt createdAt updatedAt author { login }
          }
        }
      }
    `;
        const data = await graphql(query, { owner: input.owner, repo: input.repo, number: input.number });
        const n = data.repository?.pullRequest;
        if (!n)
            throw Object.assign(new Error('Not Found'), { status: 404 });
        const item = {
            id: n.id,
            number: n.number,
            title: n.title,
            body: n.body,
            state: n.state,
            is_draft: Boolean(n.isDraft),
            created_at: n.createdAt,
            updated_at: n.updatedAt,
            merged: Boolean(n.merged),
            merged_at: n.mergedAt || null,
            ...(input.include_author ? { author_login: n.author?.login || null } : {}),
        };
        return { item, meta: { rate: parseGraphqlRate(data.rateLimit) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function getPrStatusSummary(input) {
    try {
        const graphql = createGraphqlClient();
        const query = /* GraphQL */ `
      query GetPrStatusSummary($owner: String!, $repo: String!, $number: Int!, $limit_contexts: Int = 10) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          pullRequest(number: $number) {
            commits(last: 1) {
              nodes {
                commit {
                  oid
                  statusCheckRollup {
                    state
                    contexts(first: $limit_contexts) {
                      nodes {
                        __typename
                        ... on CheckRun { name conclusion }
                        ... on StatusContext { context state }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    `;
        const data = await graphql(query, {
            owner: input.owner,
            repo: input.repo,
            number: input.number,
            limit_contexts: input.limit_contexts ?? 10,
        });
        const rollup = data.repository?.pullRequest?.commits?.nodes?.[0]?.commit?.statusCheckRollup;
        const contexts = rollup?.contexts?.nodes || [];
        let success = 0, pending = 0, failure = 0;
        const failing_contexts = [];
        for (const c of contexts) {
            if (c.__typename === 'CheckRun') {
                const concl = (c.conclusion || '').toUpperCase();
                if (concl === 'SUCCESS' || concl === 'NEUTRAL' || concl === 'SKIPPED')
                    success++;
                else if (concl === 'PENDING' || concl === 'QUEUED' || concl === 'IN_PROGRESS')
                    pending++;
                else {
                    failure++;
                    if (input.include_failing_contexts)
                        failing_contexts.push(c.name);
                }
            }
            else if (c.__typename === 'StatusContext') {
                const state = (c.state || '').toUpperCase();
                if (state === 'SUCCESS')
                    success++;
                else if (state === 'PENDING')
                    pending++;
                else {
                    failure++;
                    if (input.include_failing_contexts)
                        failing_contexts.push(c.context);
                }
            }
        }
        let overall = (rollup?.state || '').toUpperCase();
        if (!overall) {
            overall = failure > 0 ? 'FAILURE' : pending > 0 ? 'PENDING' : 'SUCCESS';
        }
        return {
            item: { overall_state: overall, counts: { success, pending, failure }, ...(input.include_failing_contexts ? { failing_contexts } : {}) },
            meta: { rate: parseGraphqlRate(data.rateLimit) },
        };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function listPrCommentsPlain(input) {
    try {
        const graphql = createGraphqlClient();
        const query = /* GraphQL */ `
      query ListPrComments($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          pullRequest(number: $number) {
            comments(first: $first, after: $after) {
              nodes { id body createdAt updatedAt author { login } }
              pageInfo { hasNextPage endCursor }
            }
          }
        }
      }
    `;
        const data = await graphql(query, {
            owner: input.owner,
            repo: input.repo,
            number: input.number,
            first: input.limit ?? 30,
            after: input.cursor,
        });
        const comments = data.repository?.pullRequest?.comments;
        const nodes = comments?.nodes || [];
        const pageInfo = comments?.pageInfo || {};
        const items = nodes.map((n) => ({
            id: n.id,
            body: n.body,
            created_at: n.createdAt,
            updated_at: n.updatedAt,
            ...(input.include_author ? { author_login: n.author?.login || null } : {}),
        }));
        return { items, meta: { next_cursor: pageInfo.endCursor || null, has_more: Boolean(pageInfo.hasNextPage), rate: parseGraphqlRate(data.rateLimit) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function listPrReviewCommentsPlain(input) {
    try {
        const graphql = createGraphqlClient();
        const query = /* GraphQL */ `
      query ListPrReviewComments($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          pullRequest(number: $number) {
            reviewComments(first: $first, after: $after) {
              nodes {
                id
                body
                createdAt
                updatedAt
                author { login }
                path
                diffHunk
                line
                startLine
                side
                startSide
                originalLine
                originalStartLine
                commit { oid }
                originalCommit { oid }
                pullRequestReviewThread { path line startLine side startSide }
              }
              pageInfo { hasNextPage endCursor }
            }
          }
        }
      }
    `;
        const data = await graphql(query, {
            owner: input.owner,
            repo: input.repo,
            number: input.number,
            first: input.limit ?? 30,
            after: input.cursor,
        });
        const root = data.repository?.pullRequest?.reviewComments;
        const nodes = root?.nodes || [];
        const pageInfo = root?.pageInfo || {};
        const items = nodes.map((n) => {
            const base = {
                id: n.id,
                body: n.body,
                created_at: n.createdAt,
                updated_at: n.updatedAt,
                ...(input.include_author ? { author_login: n.author?.login || null } : {}),
            };
            if (input.include_location) {
                const thread = n.pullRequestReviewThread || {};
                Object.assign(base, {
                    path: n.path || thread.path || undefined,
                    line: n.line ?? thread.line ?? undefined,
                    start_line: n.startLine ?? thread.startLine ?? undefined,
                    side: n.side ?? thread.side ?? undefined,
                    start_side: n.startSide ?? thread.startSide ?? undefined,
                    original_line: n.originalLine ?? undefined,
                    original_start_line: n.originalStartLine ?? undefined,
                    diff_hunk: n.diffHunk ?? undefined,
                    commit_sha: n.commit?.oid ?? undefined,
                    original_commit_sha: n.originalCommit?.oid ?? undefined,
                });
            }
            return base;
        });
        return { items, meta: { next_cursor: pageInfo.endCursor || null, has_more: Boolean(pageInfo.hasNextPage), rate: parseGraphqlRate(data.rateLimit) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function listPrReviewsLight(input) {
    try {
        const graphql = createGraphqlClient();
        const query = /* GraphQL */ `
      query ListPrReviews($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          pullRequest(number: $number) {
            reviews(first: $first, after: $after) {
              nodes { id state submittedAt author { login } }
              pageInfo { hasNextPage endCursor }
            }
          }
        }
      }
    `;
        const data = await graphql(query, {
            owner: input.owner,
            repo: input.repo,
            number: input.number,
            first: input.limit ?? 30,
            after: input.cursor,
        });
        const root = data.repository?.pullRequest?.reviews;
        const nodes = root?.nodes || [];
        const pageInfo = root?.pageInfo || {};
        const items = nodes.map((n) => ({
            id: n.id,
            state: n.state,
            submitted_at: n.submittedAt || null,
            ...(input.include_author ? { author_login: n.author?.login || null } : {}),
        }));
        return { items, meta: { next_cursor: pageInfo.endCursor || null, has_more: Boolean(pageInfo.hasNextPage), rate: parseGraphqlRate(data.rateLimit) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function listPrCommitsLight(input) {
    try {
        const graphql = createGraphqlClient();
        const query = /* GraphQL */ `
      query ListPrCommits($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          pullRequest(number: $number) {
            commits(first: $first, after: $after) {
              nodes { commit { oid messageHeadline authoredDate author { user { login } } } }
              pageInfo { hasNextPage endCursor }
            }
          }
        }
      }
    `;
        const data = await graphql(query, {
            owner: input.owner,
            repo: input.repo,
            number: input.number,
            first: input.limit ?? 30,
            after: input.cursor,
        });
        const root = data.repository?.pullRequest?.commits;
        const nodes = root?.nodes || [];
        const pageInfo = root?.pageInfo || {};
        const items = nodes.map((n) => ({
            sha: n.commit?.oid,
            title: n.commit?.messageHeadline,
            authored_at: n.commit?.authoredDate,
            ...(input.include_author ? { author_login: n.commit?.author?.user?.login || null } : {}),
        }));
        return { items, meta: { next_cursor: pageInfo.endCursor || null, has_more: Boolean(pageInfo.hasNextPage), rate: parseGraphqlRate(data.rateLimit) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function listPrFilesLight(input) {
    try {
        const rest = createRestClient();
        const per_page = input.per_page ?? 30;
        const page = input.page ?? 1;
        const res = await rest.request('GET /repos/{owner}/{repo}/pulls/{number}/files', {
            owner: input.owner,
            repo: input.repo,
            number: input.number,
            per_page,
            page,
            headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        const items = res.data.map((f) => ({
            filename: f.filename,
            status: f.status,
            additions: f.additions,
            deletions: f.deletions,
            changes: f.changes,
            sha: f.sha,
            ...(input.include_patch ? { patch: f.patch } : {}),
        }));
        const { next_cursor, has_more } = restNextCursor(res.headers, page, per_page, items.length);
        return { items, meta: { next_cursor, has_more, rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function getPrDiff(input) {
    try {
        const rest = createRestClient();
        const res = await rest.request('GET /repos/{owner}/{repo}/pulls/{number}', {
            owner: input.owner,
            repo: input.repo,
            number: input.number,
            headers: { Accept: 'application/vnd.github.v3.diff', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        return { diff: String(res.data), meta: { rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function getPrPatch(input) {
    try {
        const rest = createRestClient();
        const res = await rest.request('GET /repos/{owner}/{repo}/pulls/{number}', {
            owner: input.owner,
            repo: input.repo,
            number: input.number,
            headers: { Accept: 'application/vnd.github.v3.patch', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        return { patch: String(res.data), meta: { rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
//# sourceMappingURL=prs.js.map