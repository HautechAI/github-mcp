import { createGraphqlClient } from '../clients/graphql.js';
import { parseGraphqlRate } from '../utils/rate.js';
import { mapError } from '../utils/errors.js';
export async function listIssues(input) {
    try {
        const graphql = createGraphqlClient();
        const states = input.state ? [input.state.toUpperCase()] : undefined;
        const filterBy = {};
        if (input.labels && input.labels.length)
            filterBy.labels = input.labels;
        if (input.creator)
            filterBy.createdBy = input.creator;
        if (input.assignee)
            filterBy.assignee = input.assignee;
        if (input.mentions)
            filterBy.mentioned = input.mentions;
        if (input.since)
            filterBy.since = input.since;
        const query = /* GraphQL */ `
      query ListIssues($owner: String!, $repo: String!, $first: Int = 30, $after: String, $states: [IssueState!], $filterBy: IssueFilters) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          issues(first: $first, after: $after, states: $states, filterBy: $filterBy) {
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
            filterBy: Object.keys(filterBy).length ? filterBy : undefined,
        });
        const nodes = data.repository?.issues?.nodes || [];
        const pageInfo = data.repository?.issues?.pageInfo || {};
        const items = nodes.map((n) => ({
            id: n.id,
            number: n.number,
            title: n.title,
            state: n.state,
            created_at: n.createdAt,
            updated_at: n.updatedAt,
            ...(input.include_author ? { author_login: n.author?.login || null } : {}),
        }));
        const meta = {
            next_cursor: pageInfo.endCursor || null,
            has_more: Boolean(pageInfo.hasNextPage),
            rate: parseGraphqlRate(data.rateLimit),
        };
        return { items, meta };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function getIssue(input) {
    try {
        const graphql = createGraphqlClient();
        const query = /* GraphQL */ `
      query GetIssue($owner: String!, $repo: String!, $number: Int!) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          issue(number: $number) {
            id number title body state createdAt updatedAt author { login }
          }
        }
      }
    `;
        const data = await graphql(query, { owner: input.owner, repo: input.repo, number: input.number });
        const n = data.repository?.issue;
        if (!n)
            throw Object.assign(new Error('Not Found'), { status: 404 });
        const item = {
            id: n.id,
            number: n.number,
            title: n.title,
            body: n.body,
            state: n.state,
            created_at: n.createdAt,
            updated_at: n.updatedAt,
            ...(input.include_author ? { author_login: n.author?.login || null } : {}),
        };
        return { item, meta: { rate: parseGraphqlRate(data.rateLimit) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function listIssueCommentsPlain(input) {
    try {
        const graphql = createGraphqlClient();
        const query = /* GraphQL */ `
      query ListIssueComments($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
        rateLimit { remaining used resetAt }
        repository(owner: $owner, name: $repo) {
          issue(number: $number) {
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
        const comments = data.repository?.issue?.comments;
        const nodes = comments?.nodes || [];
        const pageInfo = comments?.pageInfo || {};
        const items = nodes.map((n) => ({
            id: n.id,
            body: n.body,
            created_at: n.createdAt,
            updated_at: n.updatedAt,
            ...(input.include_author ? { author_login: n.author?.login || null } : {}),
        }));
        const meta = {
            next_cursor: pageInfo.endCursor || null,
            has_more: Boolean(pageInfo.hasNextPage),
            rate: parseGraphqlRate(data.rateLimit),
        };
        return { items, meta };
    }
    catch (err) {
        return mapError(err);
    }
}
//# sourceMappingURL=issues.js.map