// Offline terminal adapter. The library owns parsing and layout; source is data.
import { readFileSync } from 'node:fs';
import { renderMermaidASCII } from './beautiful-mermaid-1.1.3.mjs';

const MAX_SOURCE = 8192;
const MAX_OUTPUT = 65536;
const unsupported = source =>
  // Upstream otherwise ignores these or renders them incorrectly. Keep source.
  /^(?:graph|flowchart)\s+RL\b/im.test(source) ||
  /^\s*(?:click|callback|activate|deactivate|autonumber|create|destroy|box|links|link|accTitle|accDescr)\b/im.test(source) ||
  /^\s*direction\s+(?:RL|BT)\b/im.test(source) ||
  /%%\s*\{|@\{|^---\s*$/m.test(source);

function render({ source, width }) {
  if (typeof source !== 'string' || Buffer.byteLength(source) > MAX_SOURCE ||
      source.split('\n').length > 128 || !Number.isInteger(width) || width < 12 || width > 240) {
    return { error: 'limit' };
  }
  if (/[\x00-\x08\x0b-\x1f\x7f-\x9f\u202a-\u202e\u2066-\u2069]/u.test(source)) {
    return { error: 'unsupported' };
  }
  // Only choose the upstream renderer. This is not a second Mermaid grammar.
  const header = source.split('\n').map(line => line.trim()).find(line => line && !line.startsWith('%%')) ?? '';
  // The package also has class/ER/chart paths, but our conformance samples found
  // clipped ER labels. Keep these as source until their fidelity is verified.
  if (!/^(?:(?:graph|flowchart)\s+(?:TD|TB|LR|BT)|stateDiagram(?:-v2)?|sequenceDiagram)\s*$/i.test(header) || unsupported(source)) {
    return { error: 'unsupported' };
  }
  try {
    // Comment-only preambles are accepted by Mermaid but not all upstream paths.
    const body = source.split('\n').filter(line => !line.trim().startsWith('%%')).join('\n').trim();
    const value = renderMermaidASCII(body, {
      useAscii: false, colorMode: 'none', paddingX: 2, paddingY: 2, boxBorderPadding: 0,
    });
    const lines = value.split('\n').map(line => line.trimEnd());
    while (lines.length && !lines.at(-1)) lines.pop();
    if (!lines.length || lines.length > 256 || Buffer.byteLength(value) > MAX_OUTPUT) return { error: 'limit' };
    // Rust makes the authoritative terminal-cell width check (including Unicode).
    if (lines.some(line => [...line].length > width)) return { error: 'wide' };
    return { lines };
  } catch {
    return { error: 'unsupported' };
  }
}

try {
  const input = readFileSync(0);
  const result = input.length > 32768 ? { error: 'limit' } : render(JSON.parse(input.toString('utf8')));
  process.stdout.write(JSON.stringify(result));
} catch {
  process.stdout.write('{"error":"unsupported"}');
}
