import { graphql as graphqlImpl } from '@octokit/graphql';
import { getConfig } from '../utils/config.js';

export function createGraphqlClient() {
  const cfg = getConfig();
  const baseUrl = cfg.GITHUB_BASE_URL || 'https://api.github.com';
  const graphql = graphqlImpl.defaults({
    baseUrl: baseUrl.replace(/\/?$/, '') + '/graphql',
    headers: {
      authorization: `token ${cfg.GITHUB_TOKEN}`,
      'X-GitHub-Api-Version': '2022-11-28',
    },
  });
  return graphql;
}
