import { createRestClient } from '../clients/rest.js';
import { mapError } from '../utils/errors.js';
import { parseRestRateHeaders } from '../utils/rate.js';
import { restNextCursor } from '../utils/pagination.js';
import { addTimestamps, downloadZipAndAggregateText, tailText } from '../utils/http.js';
export async function listWorkflowsLight(input) {
    try {
        const rest = createRestClient();
        const per_page = input.per_page ?? 30;
        const page = input.page ?? 1;
        const res = await rest.request('GET /repos/{owner}/{repo}/actions/workflows', {
            owner: input.owner,
            repo: input.repo,
            per_page,
            page,
            headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        const items = (res.data.workflows || []).map((w) => ({ id: w.id, name: w.name, path: w.path, state: w.state }));
        const { next_cursor, has_more } = restNextCursor(res.headers, page, per_page, items.length);
        return { items, meta: { next_cursor, has_more, rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function listWorkflowRunsLight(input) {
    try {
        const rest = createRestClient();
        const per_page = input.per_page ?? 30;
        const page = input.page ?? 1;
        const res = await rest.request('GET /repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs', {
            owner: input.owner,
            repo: input.repo,
            workflow_id: input.workflow_id,
            status: input.status,
            branch: input.branch,
            actor: input.actor,
            event: input.event,
            created: input.created,
            head_sha: input.head_sha,
            per_page,
            page,
            headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        const items = (res.data.workflow_runs || []).map((r) => ({
            id: r.id,
            run_number: r.run_number,
            event: r.event,
            status: r.status,
            conclusion: (r.conclusion ?? null),
            head_sha: r.head_sha,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }));
        const { next_cursor, has_more } = restNextCursor(res.headers, page, per_page, items.length);
        return { items, meta: { next_cursor, has_more, rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function getWorkflowRunLight(input) {
    try {
        const rest = createRestClient();
        const res = await rest.request('GET /repos/{owner}/{repo}/actions/runs/{run_id}', {
            owner: input.owner,
            repo: input.repo,
            run_id: input.run_id,
            exclude_pull_requests: input.exclude_pull_requests ? true : undefined,
            headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        const r = res.data;
        const item = {
            id: r.id,
            run_number: r.run_number,
            event: r.event,
            status: r.status,
            conclusion: r.conclusion ?? null,
            head_sha: r.head_sha,
            created_at: r.created_at,
            updated_at: r.updated_at,
        };
        return { item, meta: { rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function listWorkflowJobsLight(input) {
    try {
        const rest = createRestClient();
        const per_page = input.per_page ?? 30;
        const page = input.page ?? 1;
        const res = await rest.request('GET /repos/{owner}/{repo}/actions/runs/{run_id}/jobs', {
            owner: input.owner,
            repo: input.repo,
            run_id: input.run_id,
            filter: input.filter,
            per_page,
            page,
            headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        const items = (res.data.jobs || []).map((j) => ({
            id: j.id,
            name: j.name,
            status: j.status,
            conclusion: j.conclusion ?? null,
            started_at: j.started_at ?? null,
            completed_at: j.completed_at ?? null,
        }));
        const { next_cursor, has_more } = restNextCursor(res.headers, page, per_page, items.length);
        return { items, meta: { next_cursor, has_more, rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function getWorkflowJobLogs(input) {
    try {
        const rest = createRestClient();
        const baseRequest = await rest.request('GET /repos/{owner}/{repo}/actions/jobs/{job_id}/logs', {
            owner: input.owner,
            repo: input.repo,
            job_id: input.job_id,
            request: { fetch: (url, opts) => globalThis.fetch(url, { ...opts, redirect: 'manual' }) },
            headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        const location = baseRequest.headers?.location || baseRequest.headers?.Location;
        if (!location)
            throw Object.assign(new Error('Missing redirect location for logs'), { status: baseRequest.status });
        let logs = await downloadZipAndAggregateText(location);
        let truncated = false;
        if (input.tail_lines && input.tail_lines > 0) {
            const t = tailText(logs, input.tail_lines);
            logs = t.text;
            truncated = t.truncated;
        }
        if (input.include_timestamps)
            logs = addTimestamps(logs);
        return { logs, truncated, meta: { rate: parseRestRateHeaders(baseRequest.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function rerunWorkflowRun(input) {
    try {
        const rest = createRestClient();
        const res = await rest.request('POST /repos/{owner}/{repo}/actions/runs/{run_id}/rerun', {
            owner: input.owner,
            repo: input.repo,
            run_id: input.run_id,
            headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        const queued_run_id = res.data?.id ?? null;
        return { ok: true, queued_run_id, meta: { rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function rerunWorkflowRunFailed(input) {
    try {
        const rest = createRestClient();
        const res = await rest.request('POST /repos/{owner}/{repo}/actions/runs/{run_id}/rerun-failed-jobs', {
            owner: input.owner,
            repo: input.repo,
            run_id: input.run_id,
            headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        const queued_run_id = res.data?.id ?? null;
        return { ok: true, queued_run_id, meta: { rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
export async function cancelWorkflowRun(input) {
    try {
        const rest = createRestClient();
        const res = await rest.request('POST /repos/{owner}/{repo}/actions/runs/{run_id}/cancel', {
            owner: input.owner,
            repo: input.repo,
            run_id: input.run_id,
            headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' },
        });
        return { ok: true, meta: { rate: parseRestRateHeaders(res.headers) } };
    }
    catch (err) {
        return mapError(err);
    }
}
//# sourceMappingURL=workflows.js.map