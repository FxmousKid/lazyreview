import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, copyFileSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const dir = mkdtempSync(join(tmpdir(), 'rw-mermaid-test-'));
copyFileSync(join(root, 'tui/src/mermaid.mjs'), join(dir, 'mermaid.mjs'));
copyFileSync(join(root, 'vendor/beautiful-mermaid-1.1.3.mjs'), join(dir, 'beautiful-mermaid-1.1.3.mjs'));
after(() => rmSync(dir, { recursive: true, force: true }));
function render(source, width = 84) {
  const result = spawnSync(process.execPath, ['--max-old-space-size=64', join(dir, 'mermaid.mjs')], {
    input: JSON.stringify({ source, width }), encoding: 'utf8', timeout: 2000, maxBuffer: 65537,
    env: { NO_COLOR: '1' },
  });
  assert.equal(result.status, 0, `helper failed: ${result.error?.code ?? result.status}`);
  return JSON.parse(result.stdout);
}

for (const [family, source, words] of [
  ['flowchart', 'flowchart TD\n PR[Pull request] --> Patch[Read patches]\n Patch --> Context[Ask companion]', ['Pull request', 'Read patches', 'Ask companion']],
  ['sequence', 'sequenceDiagram\n participant R as Reviewer\n participant AI as Companion\n R->>AI: Explain these changes\n AI-->>R: Context and tradeoffs', ['Reviewer', 'Companion', 'Explain these changes', 'Context and tradeoffs']],
  ['state', 'stateDiagram-v2\n [*] --> Pending\n Pending --> Reviewed\n Reviewed --> [*]', ['Pending', 'Reviewed']],
]) {
  test(`${family} renders labels and Unicode geometry without terminal escapes`, () => {
    const result = render(source);
    assert.ok(result.lines?.length, JSON.stringify(result));
    const text = result.lines.join('\n');
    for (const word of words) assert.ok(text.includes(word), word);
    assert.match(text, /[┌╭│]/u);
    assert.doesNotMatch(text, /[\x00-\x09\x0b-\x1f\x7f]/u);
    assert.ok(result.lines.every(line => [...line].length <= 84));
  });
}

test('vertical flow stays readable at 24 columns; wide sequence keeps source', () => {
  const narrow = render('flowchart TD\n A[Review] --> B[Understand]\n B --> C[Comment]', 24);
  assert.ok(narrow.lines?.length);
  assert.ok(narrow.lines.every(line => line.length <= 24));
  assert.equal(render('sequenceDiagram\n Alice->>Bob: This label needs a wider reading column', 24).error, 'wide');
});

test('unsupported family and known lossy upstream forms fall back', () => {
  for (const source of [
    'pie\n "Docs" : 10', 'gantt\n title Build', 'flowchart RL\n A --> B',
    'classDiagram\n class Review', 'erDiagram\n REVIEW ||--o{ COMMENT : contains',
    'flowchart TD\n A --> B\n click A "https://example.invalid"',
    'sequenceDiagram\n autonumber\n A->>B: Hello',
    'sequenceDiagram\n activate B\n A->>B: Hello',
    '%%{init: {securityLevel: "loose"}}%%\nflowchart TD\n A-->B',
    '---\ntitle: Custom config\n---\nflowchart TD\n A-->B',
    'flowchart TD\n A@{img: "https://example.invalid/image.png"}',
  ]) assert.equal(render(source).error, 'unsupported', source);
});

test('source and output limits fail closed', () => {
  assert.equal(render('x'.repeat(8193)).error, 'limit');
  assert.equal(render('graph TD\n' + 'A-->B\n'.repeat(129)).error, 'limit');
  assert.equal(render('graph TD\n A-->B', 8).error, 'limit');
  assert.equal(render('graph TD\n A[\u001b]-->B').error, 'unsupported');
  assert.equal(render('graph TD\n A[\u202e]-->B').error, 'unsupported');
});

test('the vendored text renderer has no runtime imports or dynamic code execution', () => {
  const vendor = readFileSync(join(root, 'vendor/beautiful-mermaid-1.1.3.mjs'), 'utf8');
  assert.doesNotMatch(vendor, /\bimport\s*(?:\(|['"{*])/);
  assert.doesNotMatch(vendor, /\b(?:eval|fetch|require)\s*\(|\bnew\s+Function\s*\(/);
});
