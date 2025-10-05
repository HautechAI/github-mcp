import { z } from 'zod';
import { paginationInput } from './common.js';
export const ListIssuesInput = z
    .object({
    owner: z.string(),
    repo: z.string(),
    state: z.enum(['open', 'closed', 'all']).optional(),
    labels: z.array(z.string()).optional(),
    creator: z.string().optional(),
    assignee: z.string().optional(),
    mentions: z.string().optional(),
    since: z.string().datetime().optional(),
    sort: z.enum(['created', 'updated', 'comments']).optional(),
    direction: z.enum(['asc', 'desc']).optional(),
    include_author: z.boolean().optional(),
    ...paginationInput,
})
    .strict();
export const GetIssueInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int(), include_author: z.boolean().optional() })
    .strict();
export const ListIssueCommentsInput = z
    .object({ owner: z.string(), repo: z.string(), number: z.number().int(), include_author: z.boolean().optional(), ...paginationInput })
    .strict();
//# sourceMappingURL=issues.js.map