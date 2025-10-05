import { RateMeta } from './types.js';

export function parseRestRateHeaders(headers: Record<string, any>): RateMeta | undefined {
  const remaining = headers['x-ratelimit-remaining'] ?? headers['X-RateLimit-Remaining'];
  const used = headers['x-ratelimit-used'] ?? headers['X-RateLimit-Used'];
  const reset = headers['x-ratelimit-reset'] ?? headers['X-RateLimit-Reset'];
  if (remaining == null && used == null && reset == null) return undefined;
  const reset_at = reset ? new Date(Number(reset) * 1000).toISOString() : undefined;
  return {
    remaining: remaining != null ? Number(remaining) : 0,
    used: used != null ? Number(used) : 0,
    reset_at,
  };
}

export function parseGraphqlRate(rateLimit?: {
  remaining?: number;
  used?: number;
  resetAt?: string;
}): RateMeta | undefined {
  if (!rateLimit) return undefined;
  const { remaining = 0, used = 0, resetAt } = rateLimit as any;
  return { remaining, used, reset_at: resetAt };
}
