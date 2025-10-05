import { Octokit } from '@octokit/rest';
import { retry } from '@octokit/plugin-retry';
import { throttling } from '@octokit/plugin-throttling';
import { paginateRest } from '@octokit/plugin-paginate-rest';
import fetch from 'cross-fetch';
import { getConfig } from '../utils/config.js';

const MyOctokit = Octokit.plugin(retry, throttling, paginateRest);

export function createRestClient() {
  const cfg = getConfig();
  const baseUrl = cfg.GITHUB_BASE_URL || 'https://api.github.com';
  const client = new MyOctokit({
    auth: cfg.GITHUB_TOKEN,
    baseUrl,
    request: {
      headers: {
        'X-GitHub-Api-Version': '2022-11-28',
        Accept: 'application/vnd.github+json',
      },
    },
    throttle: {
      onRateLimit: (retryAfter, options, octokit, retryCount) => {
        if (retryCount < 2) return true;
        return false;
      },
      onSecondaryRateLimit: () => true,
    },
  });
  return client;
}
