#!/usr/bin/env node
import { readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const ATTRIBUTES = /^\s+style=(?:"([^"]*)"|'([^']*)')\s*$/;
const STYLE = /^(?:(?:color|background):#[\da-fA-F]{6};|font-weight:700;|text-decoration:underline;|font-style:italic;)*$/;

// A deterministic presentation of the exporter's actual shortcut sequence. The
// browser and MP4 use this same timeline; neither records the system keyboard.
export function keycast(frame) {
  const keys = frame.kind === 'card' ? [] : frame.keys;
  const spacing = keys.length < 2 ? 0 : Math.min(450, Math.max(0, (frame.duration - 750) / (keys.length - 1)));
  const labels = { Enter: '↵', Return: '↵', Escape: 'esc', Backspace: '⌫', Tab: '⇥', Space: 'space', 'Shift+N': '⇧ N', 'Ctrl+K': '⌃ K' };
  const events = keys.map((key, i) => ({
    at: 100 + i * spacing,
    key,
    label: labels[key] || key,
    kind: /drag|mouse|click/i.test(key) ? 'gesture' : 'key',
  }));
  return { events, end: events.length ? Math.min(frame.duration - 150, events.at(-1).at + 2200) : 0, fade: 200 };
}

export function presentation(data) {
  validate(data);
  return { ...data, frames: data.frames.map(frame => ({ ...frame, keycast: keycast(frame) })) };
}

export function validate(data) {
  if (data?.version !== 1 || !Number.isInteger(data.cols) || !Number.isInteger(data.rows) || data.cols < 40 || data.cols > 300 || data.rows < 15 || data.rows > 100) throw new Error('Expected version 1 and a bounded terminal size.');
  if (!Array.isArray(data.frames) || !data.frames.length || data.frames.length > 100) throw new Error('Expected 1–100 recorded terminal frames.');
  const ids = new Set();
  for (const frame of data.frames) {
    if (!/^[a-z0-9_-]+$/i.test(frame.id) || ids.has(frame.id)) throw new Error('Each frame requires a unique simple id.');
    ids.add(frame.id);
    if (frame.kind != null && frame.kind !== 'card') throw new Error(`${frame.id}: unsupported presentation kind.`);
    for (const key of ['title', 'caption', 'chapter']) if (typeof frame[key] !== 'string' || !frame[key].trim() || frame[key].length > 400) throw new Error(`${frame.id}: ${key} must be concise nonempty text.`);
    if (typeof frame.html !== 'string' || !frame.html || frame.html.length > 4_000_000) throw new Error(`${frame.id}: missing or oversized terminal render.`);
    if (!Number.isFinite(frame.duration) || frame.duration < 500 || frame.duration > 30_000) throw new Error(`${frame.id}: duration must be 500–30000 milliseconds.`);
    if (!Array.isArray(frame.keys) || frame.keys.length > 30 || frame.keys.some(key => typeof key !== 'string' || !key || key.length > 50)) throw new Error(`${frame.id}: keys must be a short sequence.`);
    if (frame.focus != null && (!Array.isArray(frame.focus) || frame.focus.length !== 4 || frame.focus.some(n => !Number.isFinite(n) || n < 0) || frame.focus[2] < 1 || frame.focus[3] < 1 || frame.focus[0] + frame.focus[2] > data.cols || frame.focus[1] + frame.focus[3] > data.rows)) throw new Error(`${frame.id}: focus must fit the terminal cell rectangle.`);
    for (const match of frame.html.matchAll(/<([^>]*)>/g)) {
      const tag = match[1];
      if (/^\/?pre$|^\/span$/.test(tag)) continue;
      if (!tag.startsWith('span ')) throw new Error(`${frame.id}: only native pre/span terminal markup is allowed.`);
      const attributes = tag.slice(4).match(ATTRIBUTES);
      if (!attributes || !STYLE.test(attributes[1] ?? attributes[2])) throw new Error(`${frame.id}: unexpected terminal style.`);
    }
    // Native text is escaped before export. A remaining '<' must begin one of the
    // exact tags above; forbid ambiguous markup rather than repairing it.
    if (frame.html.replace(/<[^>]*>/g, '').includes('<')) throw new Error(`${frame.id}: unescaped terminal text.`);
  }
  return data;
}

export async function build(input = resolve(here, 'frames.json'), output = resolve(here, 'lazyreview-demo.html')) {
  const [template, css, js, raw] = await Promise.all([
    readFile(resolve(here, 'index.html'), 'utf8'), readFile(resolve(here, 'style.css'), 'utf8'),
    readFile(resolve(here, 'player.js'), 'utf8'), readFile(input, 'utf8'),
  ]);
  const data = presentation(JSON.parse(raw));
  const serialized = JSON.stringify(data).replace(/</g, '\\u003c').replace(/\u2028/g, '\\u2028').replace(/\u2029/g, '\\u2029');
  const html = template.replace('<!-- DEMO_STYLE -->', `<style>\n${css}\n</style>`)
    .replace('<!-- DEMO_DATA -->', () => serialized)
    .replace('<!-- DEMO_SCRIPT -->', `<script>\n${js}\n</script>`);
  await writeFile(output, html);
  return { output, frames: data.frames.length, durationSeconds: data.frames.reduce((n, frame) => n + frame.duration, 0) / 1000, bytes: Buffer.byteLength(html) };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv[2] === '--timeline') {
    console.log(JSON.stringify(presentation(JSON.parse(await readFile(resolve(process.argv[3] || resolve(here, 'frames.json')), 'utf8')))));
  } else console.log(JSON.stringify(await build(process.argv[2] && resolve(process.argv[2]), process.argv[3] && resolve(process.argv[3]))));
}
