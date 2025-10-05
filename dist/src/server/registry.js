import { ListIssuesInput, GetIssueInput, ListIssueCommentsInput, } from '../schemas/issues.js';
import { ListPullRequestsInput, GetPullRequestInput, GetPrStatusSummaryInput, ListPrCommentsInput, ListPrReviewCommentsInput, ListPrReviewsInput, ListPrCommitsInput, ListPrFilesInput, GetPrDiffInput, GetPrPatchInput, } from '../schemas/prs.js';
import { ListWorkflowsInput, ListWorkflowRunsInput, GetWorkflowRunInput, ListWorkflowJobsInput, GetWorkflowJobLogsInput, SimpleRunInput, } from '../schemas/workflows.js';
import * as issues from '../handlers/issues.js';
import * as prs from '../handlers/prs.js';
import * as workflows from '../handlers/workflows.js';
export function getTools() {
    return [
        {
            name: 'list_issues',
            inputSchema: ListIssuesInput,
            handler: issues.listIssues,
        },
        { name: 'get_issue', inputSchema: GetIssueInput, handler: issues.getIssue },
        {
            name: 'list_issue_comments_plain',
            inputSchema: ListIssueCommentsInput,
            handler: issues.listIssueCommentsPlain,
        },
        { name: 'list_pull_requests', inputSchema: ListPullRequestsInput, handler: prs.listPullRequests },
        { name: 'get_pull_request', inputSchema: GetPullRequestInput, handler: prs.getPullRequest },
        { name: 'get_pr_status_summary', inputSchema: GetPrStatusSummaryInput, handler: prs.getPrStatusSummary },
        { name: 'list_pr_comments_plain', inputSchema: ListPrCommentsInput, handler: prs.listPrCommentsPlain },
        {
            name: 'list_pr_review_comments_plain',
            inputSchema: ListPrReviewCommentsInput,
            handler: prs.listPrReviewCommentsPlain,
        },
        { name: 'list_pr_reviews_light', inputSchema: ListPrReviewsInput, handler: prs.listPrReviewsLight },
        { name: 'list_pr_commits_light', inputSchema: ListPrCommitsInput, handler: prs.listPrCommitsLight },
        { name: 'list_pr_files_light', inputSchema: ListPrFilesInput, handler: prs.listPrFilesLight },
        { name: 'get_pr_diff', inputSchema: GetPrDiffInput, handler: prs.getPrDiff },
        { name: 'get_pr_patch', inputSchema: GetPrPatchInput, handler: prs.getPrPatch },
        { name: 'list_workflows_light', inputSchema: ListWorkflowsInput, handler: workflows.listWorkflowsLight },
        { name: 'list_workflow_runs_light', inputSchema: ListWorkflowRunsInput, handler: workflows.listWorkflowRunsLight },
        { name: 'get_workflow_run_light', inputSchema: GetWorkflowRunInput, handler: workflows.getWorkflowRunLight },
        { name: 'list_workflow_jobs_light', inputSchema: ListWorkflowJobsInput, handler: workflows.listWorkflowJobsLight },
        { name: 'get_workflow_job_logs', inputSchema: GetWorkflowJobLogsInput, handler: workflows.getWorkflowJobLogs },
        { name: 'rerun_workflow_run', inputSchema: SimpleRunInput, handler: workflows.rerunWorkflowRun },
        { name: 'rerun_workflow_run_failed', inputSchema: SimpleRunInput, handler: workflows.rerunWorkflowRunFailed },
        { name: 'cancel_workflow_run', inputSchema: SimpleRunInput, handler: workflows.cancelWorkflowRun },
    ];
}
//# sourceMappingURL=registry.js.map