import { z } from 'zod';
import { paginationInput, restPaginationInput } from './common.js';
export const ListPullRequestsInput = z
    .object({ owner: z.string(), repo: z.string(), state: z.enum(['open', 'closed', 'all']).optional(), base: z.string().optional(), head: z.string().optional(), include_author: z.boolean().optional(), ...paginationInput })
    .strict();
export const GetPullRequestInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int(), include_author: z.boolean().optional() })
    .strict();
export const GetPrStatusSummaryInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int(), include_failing_contexts: z.boolean().optional(), limit_contexts: z.number().int().min(1).max(100).default(10).optional() })
    .strict();
export const ListPrCommentsInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int(), include_author: z.boolean().optional(), ...paginationInput })
    .strict();
export const ListPrReviewCommentsInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int(), include_author: z.boolean().optional(), include_location: z.boolean().optional(), ...paginationInput })
    .strict();
export const ListPrReviewsInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int(), include_author: z.boolean().optional(), ...paginationInput })
    .strict();
export const ListPrCommitsInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int(), include_author: z.boolean().optional(), ...paginationInput })
    .strict();
export const ListPrFilesInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int(), include_patch: z.boolean().optional(), ...restPaginationInput })
    .strict();
export const GetPrDiffInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int() })
    .strict();
export const GetPrPatchInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int() })
    .strict();
//# sourceMappingURL=prs.js.map