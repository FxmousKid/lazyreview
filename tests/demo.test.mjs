import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { validate, build, presentation, keycast } from '../demo/build.mjs';
const frame = { id: 'start', title: 'Review your PRs.', caption: 'A real terminal.', chapter: 'Review', keys: ['g', 'a'], duration: 6000, focus: [0, 0, 80, 30], html: "<pre><span style='color:#e6edf3;background:#0d1117;font-weight:700;'>Review &lt;code&gt;</span>\n</pre>" };
const fixture = (patch = {}) => ({ version: 1, cols: 180, rows: 44, frames: [{ ...frame, ...patch }] });
test('accepts native terminal spans and bounded focus cells', () => assert.equal(validate(fixture()).frames[0].id, 'start'));
test('rejects scripts, event handlers, unsafe CSS and unescaped source', () => {
  for (const html of ['<script>alert(1)</script>', '<img src=x onerror=alert(1)>', '<span onclick="alert(1)">x</span>', '<span style="background:url(https://example.com)">x</span>', '<span style="color:#ffffff;position:fixed;">x</span>', 'let x = a < b']) assert.throws(() => validate(fixture({ html })));
});
test('accepts title cards and rejects unknown presentation types', () => {
  assert.equal(validate(fixture({kind:'card'})).frames[0].kind, 'card');
  assert.throws(() => validate(fixture({kind:'iframe'})));
});
test('rejects ambiguous frame identity, invalid timing, or out of bounds crop', () => {
  assert.throws(() => validate({ ...fixture(), frames: [frame, frame] }));
  for (const duration of [-1, 0, Infinity, 100_000]) assert.throws(() => validate(fixture({ duration })));
  assert.throws(() => validate(fixture({ focus: [160, 0, 80, 30] })));
});
test('builds a portable page with inert embedded source data', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'rw-demo-'));
  try {
    const input = join(dir, 'frames.json'), output = join(dir, 'demo.html');
    await writeFile(input, JSON.stringify(fixture({ caption: '</script><script>alert(1)</script>' })));
    const result = await build(input, output);
    const html = await readFile(output, 'utf8');
    assert.equal(result.frames, 1); assert.equal(result.durationSeconds, 6);
    assert.ok(!html.includes('<!-- DEMO_'));
    assert.ok(html.includes('\\u003c/script>'));
    assert.ok(!html.includes('<script>alert(1)</script>'));
    assert.ok(!/<(?:script|link|img)[^>]+(?:src|href)=["']https?:/i.test(html));
    assert.ok(html.includes('prefers-reduced-motion'));
    assert.ok(html.includes('visibilitychange'));
    assert.ok(html.includes('id="key-overlay"'));
    assert.ok(html.includes('inset: 73% 0 auto'));
    assert.ok(!html.includes('id="keyboard"'));
    assert.ok(!html.includes('keyboardRows'));
    assert.ok(!html.includes('classList.toggle(\'pending\''));
    assert.ok(html.includes('event.at <= within'));
    assert.ok(html.includes('sequence.lastChild.remove()'));
    assert.ok(html.includes('aria-label="Recent keystrokes"'));
    assert.ok(!html.includes('AT YOUR FINGERTIPS'));
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test('recorded tour stays focused and keeps the typed question inside the camera', async () => {
  const data = validate(JSON.parse(await readFile(new URL('../demo/frames.json', import.meta.url), 'utf8')));
  assert.equal(new Set(data.frames.map(frame => frame.chapter)).size, 2);
  assert.ok(data.frames.length >= 12 && data.frames.length <= 18);
  const duration = data.frames.reduce((sum, frame) => sum + frame.duration, 0);
  assert.ok(duration >= 70_000 && duration <= 90_000);
  assert.equal(data.frames.filter(frame => frame.kind === 'card').length, 3);
  const question = data.frames.find(frame => frame.title === 'Ask about the relationship.');
  const lines = question.html.replace(/<[^>]*>/g, '').split('\n');
  const row = lines.findIndex(line => line.includes('How do these two pieces work together when results become empty?'));
  assert.ok(row >= 0);
  assert.ok(row >= question.focus[1] && row < question.focus[1] + question.focus[3], 'the camera must not hide the inline question composer');
});


test('keystroke timeline reveals only elapsed events, then holds and clears', () => {
  const cast = keycast({ ...frame, keys: ['g', 'a'] });
  const visible = time => cast.events.filter(event => event.at <= time && time < cast.end).map(event => event.label);
  assert.deepEqual(visible(0), []);
  assert.deepEqual(visible(100), ['g']);
  assert.deepEqual(visible(549), ['g']);
  assert.deepEqual(visible(550), ['g', 'a']);
  assert.deepEqual(visible(2500), ['g', 'a']);
  assert.deepEqual(visible(2750), []);
  assert.equal(cast.fade, 200);
  assert.deepEqual(keycast({ ...frame, kind: 'card' }).events, []);
});

test('keystroke labels preserve chord identity and fit short scenes', () => {
  const cast = keycast({ ...frame, keys: ['Shift+N', 'Ctrl+K', 'Enter', 'Drag to select'], duration: 2000 });
  assert.deepEqual(cast.events.map(event => event.label), ['⇧ N', '⌃ K', '↵', 'Drag to select']);
  assert.equal(cast.events[0].key, 'Shift+N');
  assert.equal(cast.events.at(-1).kind, 'gesture');
  assert.ok(cast.events.at(-1).at < cast.end - cast.fade);
  assert.ok(cast.end < 2000);
  const data = presentation(fixture());
  assert.deepEqual(data.frames[0].keycast, keycast(frame));
  assert.equal(data.frames[0].html, frame.html);
});
