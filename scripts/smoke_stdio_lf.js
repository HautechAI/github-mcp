#!/usr/bin/env node
// Minimal stdio smoke test: send initialize + tools/list using LF-only header termination
const { spawn } = require('node:child_process');

const bin = process.argv[2] || './target/release/github-mcp';

function frame(obj) {
  const payload = Buffer.from(JSON.stringify(obj), 'utf8');
  // LF-only header termination intentionally
  const header = Buffer.from(`Content-Length: ${payload.length}\n\n`, 'utf8');
  return Buffer.concat([header, payload]);
}

const p = spawn(bin, [], { stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, MCP_DIAG_LOG: './diag-smoke.log' } });

let stdout = Buffer.alloc(0);
let stderr = '';
p.stdout.on('data', d => { stdout = Buffer.concat([stdout, d]); });
p.stderr.on('data', d => { stderr += d.toString(); });

function readFrame(buf) {
  // Expect CRLF headers from server; search for \r\n\r\n
  const sep = Buffer.from('\r\n\r\n');
  const idx = buf.indexOf(sep);
  if (idx === -1) return null;
  const header = buf.slice(0, idx).toString('utf8');
  const rest = buf.slice(idx + sep.length);
  const m = /Content-Length:\s*(\d+)/i.exec(header);
  if (!m) throw new Error('Missing Content-Length in response');
  const len = parseInt(m[1], 10);
  if (rest.length < len) return null;
  const body = rest.slice(0, len).toString('utf8');
  const remaining = rest.slice(len);
  return { body: JSON.parse(body), remaining };
}

function waitFrame(timeoutMs = 5000) {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    function tick() {
      const f = readFrame(stdout);
      if (f) return resolve(f);
      if (Date.now() - start > timeoutMs) return reject(new Error('timeout waiting for frame'));
      setTimeout(tick, 10);
    }
    tick();
  });
}

async function run() {
  // Send initialize
  p.stdin.write(frame({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} }));
  const init = await waitFrame();
  console.log('[smoke] initialize ok:', init.body && init.body.result && init.body.result.protocolVersion);

  // Send tools/list
  p.stdin.write(frame({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} }));
  const list = await waitFrame();
  const hasPing = Array.isArray(list.body.result && list.body.result.tools) && list.body.result.tools.find(t => t.name === 'ping');
  console.log('[smoke] tools/list ok, has ping:', !!hasPing);
  process.exit(hasPing ? 0 : 2);
}

run().catch(err => {
  console.error('[smoke] error:', err);
  console.error('[smoke] stderr:', stderr);
  process.exit(1);
});

