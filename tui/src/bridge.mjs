// Private stdio transport: no listener, no credentials in argv or responses.
import { createInterface } from 'node:readline';
import { execFileSync } from 'node:child_process';
import { executeGithub } from './github.ts';
import { sample } from './sample.ts';
import { categorize, parsePatch, selectedLines, splitRows } from './review.ts';
import { discoverPullRequests } from './discovery.mjs';
import { discoverRepositories, discoverHistory } from './repositories.mjs';
import { complete } from './completion.mjs';
import { prepareComment } from './comment.mjs';
import { syntaxSources, readSnapshotContent } from './syntax.mjs';
let current = structuredClone(sample);
let token;
let sampleSequence = Date.now();
function githubToken() {
  if (token !== undefined) return token;
  token = process.env.GH_TOKEN || process.env.GITHUB_TOKEN || '';
  if (!token) { try { token = execFileSync('gh', ['auth','token','--hostname','github.com'], { encoding:'utf8', stdio:['ignore','pipe','ignore'], timeout:10000 }).trim(); } catch {} }
  return token;
}
const normalize = pr => ({ ...pr, files: pr.files.map(f => ({ ...f, category: categorize(f.filename), lines: parsePatch(f.patch), splitRows: splitRows(parsePatch(f.patch)) })) });
async function handle(input) {
  const { operation } = input;
  if (operation === 'syntax') return syntaxSources(input, sample, request => readSnapshotContent(request, githubToken()));
  if (operation === 'suggest') { try { return await complete(input, githubToken()); } catch { return {id:input.id,url:input.url,kind:input.kind,query:input.query,items:[],error:'Suggestions unavailable; continue typing.'}; } }
  if (operation === 'repositories') { token = undefined; return discoverRepositories(input, githubToken()); }
  if (operation === 'history') { token = undefined; return discoverHistory(input, githubToken()); }
  if (operation === 'discover') { token = undefined; return discoverPullRequests(input, githubToken()); }
  if (operation === 'sample') { current = structuredClone(sample); return normalize(current); }
  if (operation === 'load') {
    current = await executeGithub({ operation:'load', url:input.url }, githubToken());
    return normalize(current);
  }
  if (operation === 'attach') {
    const file = current.files.find(f => f.filename === input.path);
    if (!file) throw new Error('Select a changed file.');
    const side = input.side || (file.status === 'removed' ? 'LEFT' : 'RIGHT');
    let content, commit = side === 'LEFT' ? current.base : current.head;
    if (input.kind === 'lines') {
      const lines = selectedLines(file, input.selection);
      if (!lines.length) throw new Error('Select returned patch lines.');
      content = lines.map(l => `${side === 'LEFT' ? l.old : l.new}: ${l.text}`).join('\n');
    } else if (input.kind === 'diff') content = file.patch;
    else if (current.demo) content = file.content;
    else ({ content, commit } = await executeGithub({ operation:'content', url:current.url, expectedHead:current.head, expectedBase:current.base, path:file.filename, side }, githubToken()));
    if (!content) throw new Error('GitHub did not provide text for this file.');
    return { id:`${commit}:${file.filename}:${input.kind}:${input.selection?.start || ''}:${input.selection?.end || ''}`, path:file.filename, content, commit, side, kind:input.kind, start:input.selection?.start, end:input.selection?.end, label:input.kind === 'lines' ? `L${input.selection.start}-${input.selection.end} ${side}` : input.kind === 'file' ? 'Whole file' : 'Changes' };
  }
  if (operation === 'chat') {
    if (current.demo) return { text:'SAMPLE EXPLANATION — not an AI-generated answer.\n\nFiltering produces the command list; the navigation hook tracks its active index. Check the index resets when the query changes and test empty results and wrapping.\n\nLoad a real PR and connect Claude Code with g a.' };
    throw new Error('Live TUI chat uses Claude Code. Connect it with g a.');
  }
  if (!['viewed','draft','submit','reply','resolve','conversation'].includes(operation)) throw new Error('Unknown operation.');
  if (input.expectedHead !== current.head || input.expectedBase !== current.base) throw new Error('The PR snapshot changed. Refresh before writing.');
  if (operation === 'draft') input = prepareComment(input, current);
  if (current.demo) {
    const now = new Date().toISOString();
    const comment = { id:++sampleSequence, author:'you', body:input.body || '', createdAt:now };
    if (operation === 'viewed') current.files = current.files.map(f => f.filename === input.path ? { ...f, viewed:input.viewed } : f);
    if (operation === 'draft') current.threads.push({ id:`sample-${comment.id}`,path:input.path, line:input.line,startLine:input.startLine, side:input.side,pending:true,canResolve:true,comments:[{...comment,pending:true}] });
    if (operation === 'submit') { current.threads.forEach(t => { t.pending=false; t.comments.forEach(c => c.pending=false); }); current.conversation.push({...comment,body:`${input.event}: ${input.body || ''}`}); }
    if (operation === 'reply') current.threads.find(t => t.comments[0]?.id === input.commentId)?.comments.push(comment);
    if (operation === 'resolve') { const t=current.threads.find(t => t.id === input.threadId); if(t) t.resolved=input.resolved; }
    if (operation === 'conversation') current.conversation.push(comment);
  } else {
    await executeGithub({ ...input, url:current.url }, githubToken());
    try { current = await executeGithub({ operation:'load', url:current.url }, githubToken()); }
    catch { throw new Error('Action completed, but refresh failed. Press f before another action.'); }
  }
  return normalize(current);
}
for await (const line of createInterface({ input:process.stdin, crlfDelay:Infinity })) {
  try { if (line.length > 300000) throw new Error('Request too large.'); const input=JSON.parse(line); process.stdout.write(JSON.stringify({ ok:true, data:await handle(input) })+'\n'); }
  catch (e) { process.stdout.write(JSON.stringify({ ok:false, error:e instanceof Error ? e.message : 'Request failed.' })+'\n'); }
}
