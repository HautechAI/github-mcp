import { z } from 'zod';
import { restPaginationInput } from './common.js';
export const ListWorkflowsInput = z
    .object({ owner: z.string(), repo: z.string(), ...restPaginationInput })
    .strict();
export const ListWorkflowRunsInput = z
    .object({
    owner: z.string(),
    repo: z.string(),
    workflow_id: z.union([z.number().int(), z.string()]),
    status: z.string().optional(),
    branch: z.string().optional(),
    actor: z.string().optional(),
    event: z.string().optional(),
    created: z.string().optional(),
    head_sha: z.string().optional(),
    ...restPaginationInput,
})
    .strict();
export const GetWorkflowRunInput = z
    .object({ owner: z.string(), repo: z.string(), run_id: z.number().int(), exclude_pull_requests: z.boolean().optional() })
    .strict();
export const ListWorkflowJobsInput = z
    .object({ owner: z.string(), repo: z.string(), run_id: z.number().int(), filter: z.enum(['latest', 'all']).optional(), ...restPaginationInput })
    .strict();
export const GetWorkflowJobLogsInput = z
    .object({ owner: z.string(), repo: z.string(), job_id: z.number().int(), tail_lines: z.number().int().optional(), include_timestamps: z.boolean().optional() })
    .strict();
export const SimpleRunInput = z
    .object({ owner: z.string(), repo: z.string(), run_id: z.number().int() })
    .strict();
//# sourceMappingURL=workflows.js.map