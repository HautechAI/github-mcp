#!/usr/bin/env node
// Persistent MCP stdio session helper.
// - Spawns the MCP server binary once and maintains a single JSON-RPC session.
// - Reads request descriptors from stdin, one JSON object per line: { method, params }.
// - Writes exactly one JSON response line to stdout per request (no extra logs).
// - All helper logs go to stderr to preserve stdout JSON-only discipline.

const { spawn } = require('node:child_process');

// Parse args: --bin <path>
function parseArgs(argv) {
  const out = { bin: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--bin') { out.bin = argv[++i]; continue; }
  }
  return out;
}

const args = parseArgs(process.argv);
if (!args.bin) {
  console.error('[e2e-session] fatal: --bin <path> is required');
  process.exit(2);
}

// Spawn server binary with inherited env (MCP_DIAG_LOG respected by server)
const child = spawn(args.bin, [], { stdio: ['pipe', 'pipe', 'pipe'], env: process.env });

let outBuf = '';
let errBuf = '';
child.stdout.on('data', (d) => { outBuf += d.toString('utf8'); });
child.stderr.on('data', (d) => { errBuf += d.toString('utf8'); });

function readJsonLine(timeoutMs = 15000, wantedId = null) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    function tryParse() {
      // Consume complete lines until we find a JSON-RPC response (optionally matching id)
      while (true) {
        const idx = outBuf.indexOf('\n');
        if (idx === -1) break;
        const line = outBuf.slice(0, idx); outBuf = outBuf.slice(idx + 1);
        if (!line.trim()) continue;
        // Ensure no framed headers appear
        if (/^\s*Content-Length:/i.test(line)) {
          return reject(new Error('Unexpected Content-Length header in stdout'));
        }
        let obj;
        try { obj = JSON.parse(line); } catch (e) {
          return reject(new Error('Invalid JSON from server: ' + line));
        }
        if (wantedId == null) return resolve(obj);
        if (obj && Object.prototype.hasOwnProperty.call(obj, 'id')) {
          if (obj.id === wantedId) return resolve(obj);
          // Not our response (e.g., notification reply), continue scanning
        }
      }
      if (Date.now() > deadline) return reject(new Error('timeout waiting for JSON line'));
      setTimeout(tryParse, 5);
    }
    tryParse();
  });
}

let nextId = 1;
async function sendRpc(method, params) {
  const id = nextId++;
  const msg = { jsonrpc: '2.0', id, method, params: params || {} };
  child.stdin.write(JSON.stringify(msg) + '\n');
  const resp = await readJsonLine(20000, id);
  return resp;
}

async function main() {
  // Perform initialize first
  try {
    const initResp = await sendRpc('initialize', {});
    if (!(initResp && initResp.result)) {
      console.error('[e2e-session] initialize: unexpected response');
      process.exit(3);
    }
  } catch (e) {
    console.error('[e2e-session] initialize failed:', e && e.message ? e.message : e);
    process.stderr.write(errBuf);
    process.exit(4);
  }

  // Interactive loop: read one JSON command per line: {method, params}
  let inBuf = '';
  process.stdin.on('data', async (chunk) => {
    inBuf += chunk.toString('utf8');
    while (true) {
      const idx = inBuf.indexOf('\n');
      if (idx === -1) break;
      const line = inBuf.slice(0, idx); inBuf = inBuf.slice(idx + 1);
      if (!line.trim()) continue;
      let cmd;
      try { cmd = JSON.parse(line); } catch (e) {
        console.error('[e2e-session] invalid command (not JSON):', line);
        continue;
      }
      const method = String(cmd.method || '').trim();
      const params = (cmd && cmd.params) || {};
      if (!method) {
        console.error('[e2e-session] invalid command (missing method)');
        continue;
      }
      try {
        const resp = await sendRpc(method, params);
        // Print exactly one JSON line to stdout
        process.stdout.write(JSON.stringify(resp) + '\n');
      } catch (e) {
        // On failure, emit a JSON-RPC error-ish envelope so callers can still log/save
        const err = { jsonrpc: '2.0', id: null, error: { code: -32603, message: String(e && e.message ? e.message : e) } };
        process.stdout.write(JSON.stringify(err) + '\n');
      }
    }
  });

  process.stdin.on('end', () => {
    try { child.kill(); } catch {}
    process.exit(0);
  });

  child.on('exit', (code, signal) => {
    if (code !== null && code !== 0) {
      console.error(`[e2e-session] server exited with code ${code}`);
    } else if (signal) {
      console.error(`[e2e-session] server terminated by signal ${signal}`);
    }
    // Propagate exit if child dies unexpectedly
    try { process.exitCode = process.exitCode || code || 0; } catch {}
  });
}

main().catch((e) => {
  console.error('[e2e-session] fatal:', e && e.message ? e.message : e);
  try { child.kill(); } catch {}
  process.exit(10);
});

