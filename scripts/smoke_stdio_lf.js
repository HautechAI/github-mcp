#!/usr/bin/env node
// Minimal stdio smoke test: send initialize + tools/list using LF-only line endings (NDJSON)
const { spawn } = require('node:child_process');

const bin = process.argv[2] || './target/release/github-mcp';

const p = spawn(bin, [], { stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, MCP_DIAG_LOG: './diag-smoke.log' } });

let stdout = '';
let stderr = '';
p.stdout.on('data', d => { stdout += d.toString('utf8'); });
p.stderr.on('data', d => { stderr += d.toString('utf8'); });

function readJsonLine(timeoutMs = 5000) {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    function tick() {
      const idx = stdout.indexOf('\n');
      if (idx !== -1) {
        const line = stdout.slice(0, idx);
        stdout = stdout.slice(idx + 1);
        if (!line.trim()) return tick();
        // Ensure no headers appear in output
        if (/Content-Length:/i.test(line)) return reject(new Error('Unexpected Content-Length header in stdout'));
        try { return resolve(JSON.parse(line)); } catch (e) { return reject(new Error('Invalid JSON line from server: ' + line)); }
      }
      if (Date.now() - start > timeoutMs) return reject(new Error('timeout waiting for JSON line'));
      setTimeout(tick, 10);
    }
    tick();
  });
}

async function run() {
  // Send initialize as NDJSON with LF only
  p.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} }) + '\n');
  const init = await readJsonLine(10000);
  console.log('[smoke] initialize ok:', init && init.result && init.result.protocolVersion);

  // Send tools/list
  p.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} }) + '\n');
  const list = await readJsonLine(10000);
  const hasPing = Array.isArray(list.result && list.result.tools) && list.result.tools.find(t => t.name === 'ping');
  console.log('[smoke] tools/list ok, has ping:', !!hasPing, '(non-fatal)');
  process.exit(0);
}

run().catch(err => {
  console.error('[smoke] error:', err);
  console.error('[smoke] stderr:', stderr);
  process.exit(1);
});
