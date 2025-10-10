#!/usr/bin/env node
// Parse llvm-cov JSON and print a markdown table of uncovered lines for changed files.
// Env:
// - CHANGED_FILES_JSON: JSON array of file paths to include (fallback: include all .rs files in report)
// - REPO: owner/repo for links
// - COMMIT_SHA: commit SHA to link to

import fs from 'node:fs';
import path from 'node:path';

function loadJson(p) {
  try {
    return JSON.parse(fs.readFileSync(p, 'utf8'));
  } catch (e) {
    console.error(`Failed to read ${p}:`, e.message);
    process.exit(1);
  }
}

const reportPath = process.argv[2] || 'coverage.json';
const report = loadJson(reportPath);

const changed = (() => {
  const raw = process.env.CHANGED_FILES_JSON || '[]';
  try {
    const arr = JSON.parse(raw);
    if (Array.isArray(arr)) return new Set(arr.map(String));
  } catch (_) {}
  return null; // include all
})();

const repo = process.env.REPO || '';
const sha = process.env.COMMIT_SHA || '';

// llvm-cov JSON schema: { data: [ { files: [ { filename, segments: [ [line, col, count, hasCount, ?] ] } ] } ] }
// We approximate uncovered lines as lines with count === 0 across all segments.

function segmentsToUncoveredLines(segments) {
  // segments sorted; each entry is [line, col, count, hasCount, ?]
  const uncovered = new Set();
  for (const seg of segments) {
    const [line, _col, count, hasCount] = seg;
    if (hasCount && count === 0) {
      uncovered.add(line);
    }
  }
  return [...uncovered].sort((a,b)=>a-b);
}

function linkFor(file, line) {
  if (!repo || !sha) return `${file}:${line}`;
  // GitHub blob link to specific line
  return `[${line}](https://github.com/${repo}/blob/${sha}/${file}#L${line})`;
}

const files = [];
for (const unit of report.data || []) {
  for (const f of unit.files || []) {
    const filename = f.filename.replace(/^\.\/?/, '');
    if (changed && !changed.has(filename)) continue;
    if (!filename.endsWith('.rs')) continue;
    const lines = segmentsToUncoveredLines(f.segments || []);
    if (lines.length === 0) continue;
    files.push({ filename, lines });
  }
}

if (files.length === 0) {
  console.log('No uncovered lines in changed Rust files.');
  process.exit(0);
}

console.log('| File | Uncovered lines |');
console.log('|------|------------------|');
for (const f of files) {
  const lineLinks = f.lines.slice(0, 200).map(l => linkFor(f.filename, l)).join(', ');
  console.log(`| ${f.filename} | ${lineLinks} |`);
}

