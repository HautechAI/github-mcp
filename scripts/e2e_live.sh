#!/usr/bin/env bash
set -euo pipefail

# Live E2E harness for github-mcp using @modelcontextprotocol/inspector-cli.
# - Validates MCP envelopes and key fields for each tool against a seeded repo.
# - Read-mostly by default; mutation tools gated by E2E_ENABLE_MUTATIONS=true.
# - Skips gracefully when fixtures are absent.
#
# Usage:
#   bash ./scripts/e2e_live.sh
#   E2E_ENABLE_MUTATIONS=true bash ./scripts/e2e_live.sh
#
# CI usage (with Doppler):
#   doppler run -p github-mcp -c dev -- bash ./scripts/e2e_live.sh

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
BIN_PATH=${BIN_PATH:-"$ROOT_DIR/target/release/github-mcp"}
OWNER=${E2E_OWNER:-"HautechAI"}
REPO=${E2E_REPO:-"github-mcp-test-repo"}
ISSUE_NUM=${E2E_ISSUE_NUM:-10}
PR_NUM=${E2E_PR_NUM:-9}
WORKFLOW_FILE=${E2E_WORKFLOW_FILE:-".github/workflows/e2e-smoke.yml"}
ENABLE_MUTATIONS=${E2E_ENABLE_MUTATIONS:-"false"}
LOG=${E2E_LOG_PATH:-"$ROOT_DIR/mcp-e2e.log"}
export MCP_DIAG_LOG=${MCP_DIAG_LOG:-"$ROOT_DIR/mcp-diag.log"}

echo "[e2e] github-mcp E2E starting" | tee "$LOG" >&2
echo "[e2e] binary: $BIN_PATH" | tee -a "$LOG" >&2
echo "[e2e] repo: $OWNER/$REPO issue #$ISSUE_NUM pr #$PR_NUM" | tee -a "$LOG" >&2
echo "[e2e] mutations enabled: $ENABLE_MUTATIONS" | tee -a "$LOG" >&2

require_file() {
  if [ ! -x "$BIN_PATH" ]; then
    echo "[e2e][fatal] binary not found or not executable: $BIN_PATH" | tee -a "$LOG" >&2
    exit 1
  fi
}

require_tools() {
  command -v node >/dev/null 2>&1 || { echo "[e2e][fatal] node is required" | tee -a "$LOG" >&2; exit 1; }
  command -v npx >/dev/null 2>&1 || { echo "[e2e][fatal] npx is required" | tee -a "$LOG" >&2; exit 1; }
}

inspector() {
  local method="$1"; shift
  # Try with timeout if present in CLI; fall back if older versions
  if ! npx -y @modelcontextprotocol/inspector-cli --version >/dev/null 2>&1; then
    echo "[e2e] installing inspector-cli on the fly" | tee -a "$LOG" >&2
  fi
  npx -y @modelcontextprotocol/inspector-cli --cli "$BIN_PATH" --method "$method" "$@"
}

save_json() {
  local path="$1"; shift
  tee "$path" >/dev/null
}

assert_has_field() {
  local path="$1"; shift
  local js_expr="$1"; shift
  PATH_ARG="$path" EXPR="$js_expr" node - <<'NODE'
  const fs=require('fs');
  const p=process.env.PATH_ARG;
  const expr=process.env.EXPR;
  const raw=fs.readFileSync(p,'utf8');
  let obj=JSON.parse(raw);
  const res=obj?.result ?? obj;
  const val=Function('o',`try{return ${expr};}catch(e){return undefined;}`)(res);
  if(val===undefined || val===null || (Array.isArray(val) && val.length===0)){
    console.error(`[assert] missing/empty: ${expr}`); process.exit(6);
  }
  console.log(`[assert] field OK: ${expr}`);
NODE
}

assert_envelope_ok() {
  local path="$1"; shift
  PATH_ARG="$path" node - <<'NODE'
  const fs=require('fs');
  const p=process.env.PATH_ARG;
  const raw=fs.readFileSync(p,'utf8');
  let obj; try{ obj=JSON.parse(raw);}catch(e){
    console.error(`[assert] not JSON: ${e}`); process.exit(2);
  }
  const res = obj?.result ?? obj; // inspector may include {result:...}
  const content = res?.content;
  const structured = res?.structuredContent;
  if(!Array.isArray(content) || content.length===0 || content[0]?.type!=='text'){
    console.error('[assert] invalid MCP content envelope'); process.exit(4);
  }
  if(structured == null){
    console.error('[assert] missing structuredContent'); process.exit(5);
  }
  console.log('[assert] envelope OK');
NODE
}

assert_is_error_code() {
  local path="$1"; shift
  local code="$1"; shift
  PATH_ARG="$path" CODE="$code" node - <<'NODE'
  const fs=require('fs');
  const p=process.env.PATH_ARG;
  const code=process.env.CODE;
  const raw=fs.readFileSync(p,'utf8');
  const obj=JSON.parse(raw);
  const res=obj?.result ?? obj;
  if(res?.isError!==true){ console.error('[assert] isError not true'); process.exit(7); }
  const sc=res?.structuredContent;
  if(!sc?.error?.code){ console.error('[assert] no error.code'); process.exit(8); }
  if(sc.error.code!==code){ console.error(`[assert] error.code ${sc.error.code} != ${code}`); process.exit(9); }
  console.log('[assert] error code OK');
NODE
}

# Prepare
require_file
require_tools

# tools/list (implicit handshake occurs inside inspector-cli)
echo "[e2e] tools/list" | tee -a "$LOG" >&2
inspector tools/list | save_json "$ROOT_DIR/out-tools.json"
assert_has_field "$ROOT_DIR/out-tools.json" "Array.isArray(o.tools)?o.tools.length:o.length"

# Optional: verify handshake was observed in diag log (non-fatal)
if [ -f "$MCP_DIAG_LOG" ]; then
  if grep -qi "initialize" "$MCP_DIAG_LOG"; then
    echo "[e2e] handshake observed in diag log" | tee -a "$LOG" >&2
  else
    echo "[e2e] note: handshake not found in diag log (non-fatal)" | tee -a "$LOG" >&2
  fi
fi

# Helper to call a tool with arguments and assert envelope
tool_call() {
  local name="$1"; shift
  local args_json="$1"; shift
  local out="$ROOT_DIR/out-${name}.json"
  echo "[e2e] tools/call ${name} ${args_json}" | tee -a "$LOG" >&2
  inspector tools/call --name "$name" --arguments "$args_json" | save_json "$out"
  assert_envelope_ok "$out"
}

# Ping
tool_call ping "{\"message\":\"ok\"}"
assert_has_field "$ROOT_DIR/out-ping.json" "o.structuredContent?.message"

# Issues
tool_call list_issues "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"limit\":10,\"include_author\":true}"
assert_has_field "$ROOT_DIR/out-list_issues.json" "Array.isArray(o.structuredContent?.items)?o.structuredContent.items.length:0"

tool_call get_issue "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$ISSUE_NUM,\"include_author\":true}"
assert_has_field "$ROOT_DIR/out-get_issue.json" "o.structuredContent?.issue?.number"

tool_call list_issue_comments_plain "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$ISSUE_NUM,\"limit\":10}"
assert_has_field "$ROOT_DIR/out-list_issue_comments_plain.json" "Array.isArray(o.structuredContent?.items)?o.structuredContent.items.length:0" || echo "[e2e] note: issue may have zero comments; proceeding" | tee -a "$LOG" >&2

# Negative path: non-existent issue
tool_call get_issue "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":999999}"
assert_is_error_code "$ROOT_DIR/out-get_issue.json" "not_found" || echo "[e2e] negative path: get_issue not_found assertion failed (tolerated)" | tee -a "$LOG" >&2

# PRs
tool_call list_pull_requests "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"limit\":10,\"include_author\":true}"
assert_has_field "$ROOT_DIR/out-list_pull_requests.json" "Array.isArray(o.structuredContent?.items)?o.structuredContent.items.length:0"

tool_call get_pull_request "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM,\"include_author\":true}"
assert_has_field "$ROOT_DIR/out-get_pull_request.json" "o.structuredContent?.pr?.number"

# Negative path: non-existent PR
tool_call get_pull_request "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":999999}"
assert_is_error_code "$ROOT_DIR/out-get_pull_request.json" "not_found" || echo "[e2e] negative path: get_pull_request not_found assertion failed (tolerated)" | tee -a "$LOG"

tool_call get_pr_status_summary "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM}"
assert_has_field "$ROOT_DIR/out-get_pr_status_summary.json" "Array.isArray(o.structuredContent?.statuses) ? o.structuredContent.statuses.length : (o.structuredContent?.status || o.structuredContent?.combined_state ? 1 : 0)" || echo "[e2e] status summary may be empty; proceeding" | tee -a "$LOG" >&2

tool_call list_pr_comments_plain "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM,\"limit\":10}"
assert_has_field "$ROOT_DIR/out-list_pr_comments_plain.json" "Array.isArray(o.structuredContent?.items)?o.structuredContent.items.length:0" || echo "[e2e] note: PR may have zero comments; proceeding" | tee -a "$LOG" >&2

tool_call list_pr_review_comments_plain "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM,\"limit\":10}"
assert_has_field "$ROOT_DIR/out-list_pr_review_comments_plain.json" "Array.isArray(o.structuredContent?.items)"

tool_call list_pr_review_threads_light "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM,\"limit\":10}"
assert_has_field "$ROOT_DIR/out-list_pr_review_threads_light.json" "Array.isArray(o.structuredContent?.items)"

tool_call list_pr_reviews_light "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM,\"limit\":10}"
assert_has_field "$ROOT_DIR/out-list_pr_reviews_light.json" "Array.isArray(o.structuredContent?.items)"

tool_call list_pr_commits_light "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM,\"limit\":10}"
assert_has_field "$ROOT_DIR/out-list_pr_commits_light.json" "Array.isArray(o.structuredContent?.items)?o.structuredContent.items.length:0"

tool_call list_pr_files_light "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM,\"per_page\":50,\"page\":1,\"include_patch\":true}"
assert_has_field "$ROOT_DIR/out-list_pr_files_light.json" "Array.isArray(o.structuredContent?.items)"

tool_call get_pr_diff "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM}"
assert_has_field "$ROOT_DIR/out-get_pr_diff.json" "(o.structuredContent?.diff||'').length" || echo "[e2e] get_pr_diff may be empty; proceeding" | tee -a "$LOG" >&2

tool_call get_pr_patch "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"number\":$PR_NUM}"
assert_has_field "$ROOT_DIR/out-get_pr_patch.json" "(o.structuredContent?.patch||'').length" || echo "[e2e] get_pr_patch may be empty; proceeding" | tee -a "$LOG" >&2

# Actions (REST)
tool_call list_workflows_light "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"per_page\":50,\"page\":1}"
assert_has_field "$ROOT_DIR/out-list_workflows_light.json" "Array.isArray(o.structuredContent?.items)?o.structuredContent.items.length:0" || echo "[e2e] no workflows listed; proceeding" | tee -a "$LOG" >&2

tool_call list_workflow_runs_light "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"per_page\":25,\"page\":1}"
assert_has_field "$ROOT_DIR/out-list_workflow_runs_light.json" "Array.isArray(o.structuredContent?.items)" || echo "[e2e] no workflow runs; proceeding" | tee -a "$LOG" >&2

# Try to pick the newest run id for further checks
LATEST_RUN_ID=$(node - "$ROOT_DIR/out-list_workflow_runs_light.json" <<'NODE'
const fs=require('fs');
try{
  const o=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
  const res=o?.result??o; const items=res?.structuredContent?.items||[];
  const id = items[0]?.id || null; process.stdout.write(String(id||''));
}catch{ process.stdout.write(''); }
NODE
)

if [ -n "$LATEST_RUN_ID" ]; then
  tool_call get_workflow_run_light "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"run_id\":$LATEST_RUN_ID}"
  assert_has_field "$ROOT_DIR/out-get_workflow_run_light.json" "o.structuredContent?.run?.id" || true

  tool_call list_workflow_jobs_light "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"run_id\":$LATEST_RUN_ID,\"per_page\":50,\"page\":1}"
  assert_has_field "$ROOT_DIR/out-list_workflow_jobs_light.json" "Array.isArray(o.structuredContent?.items)" || true

  JOB_ID=$(node - "$ROOT_DIR/out-list_workflow_jobs_light.json" <<'NODE'
const fs=require('fs');
try{
  const o=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
  const res=o?.result??o; const items=res?.structuredContent?.items||[];
  const id = items[0]?.id || null; process.stdout.write(String(id||''));
}catch{ process.stdout.write(''); }
NODE
)

  if [ -n "$JOB_ID" ]; then
    tool_call get_workflow_job_logs "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"job_id\":$JOB_ID,\"tail_lines\":200,\"include_timestamps\":true}"
    assert_has_field "$ROOT_DIR/out-get_workflow_job_logs.json" "(o.structuredContent?.logs||'').length" || echo "[e2e] logs not available; proceeding" | tee -a "$LOG" >&2
  else
    echo "[e2e] skip job logs (no jobs)" | tee -a "$LOG" >&2
  fi

  if [ "$ENABLE_MUTATIONS" = "true" ]; then
    echo "[e2e] mutations enabled; attempting rerun/cancel (best-effort)" | tee -a "$LOG" >&2
    inspector tools/call --name rerun_workflow_run --arguments "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"run_id\":$LATEST_RUN_ID}" | save_json "$ROOT_DIR/out-rerun_workflow_run.json" || true
    inspector tools/call --name rerun_workflow_run_failed --arguments "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"run_id\":$LATEST_RUN_ID}" | save_json "$ROOT_DIR/out-rerun_workflow_run_failed.json" || true
    inspector tools/call --name cancel_workflow_run --arguments "{\"owner\":\"$OWNER\",\"repo\":\"$REPO\",\"run_id\":$LATEST_RUN_ID}" | save_json "$ROOT_DIR/out-cancel_workflow_run.json" || true
  fi
else
  echo "[e2e] no workflow runs found; skipping downstream actions checks" | tee -a "$LOG" >&2
fi

# Review thread resolve/unresolve are gated and require a thread id; we only attempt if non-empty.
if [ "$ENABLE_MUTATIONS" = "true" ]; then
  THREAD_ID=$(node - "$ROOT_DIR/out-list_pr_review_threads_light.json" <<'NODE'
const fs=require('fs');
try{
  const o=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
  const res=o?.result??o; const items=res?.structuredContent?.items||[];
  const id = items.find(x=>x?.id && x?.isResolved===false)?.id || items[0]?.id || null; process.stdout.write(String(id||''));
}catch{ process.stdout.write(''); }
NODE
)
  if [ -n "$THREAD_ID" ] && [ "$THREAD_ID" != "null" ]; then
    inspector tools/call --name resolve_pr_review_thread --arguments "{\"thread_id\":\"$THREAD_ID\"}" | save_json "$ROOT_DIR/out-resolve_pr_review_thread.json" || true
    inspector tools/call --name unresolve_pr_review_thread --arguments "{\"thread_id\":\"$THREAD_ID\"}" | save_json "$ROOT_DIR/out-unresolve_pr_review_thread.json" || true
  else
    echo "[e2e] skip resolve/unresolve (no thread)" | tee -a "$LOG" >&2
  fi
fi

echo "[e2e] DONE" | tee -a "$LOG" >&2
