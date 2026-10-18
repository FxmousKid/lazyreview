// Opt-in native Claude grounding probe. All PR data is synthetic; no GitHub mutations.
// The description deliberately contradicts the implementation. Evidence is supplied
// automatically; neither user question commands a tool call or identifies a file.
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {performance} from 'node:perf_hooks';
import {Companion} from '../tui/src/ai-session.mjs';
if(!process.argv.includes('--live'))throw Error('Pass --live to use Claude for two bounded synthetic evidence turns');
const root=await mkdtemp(join(tmpdir(),'rw-grounding-live-'));
const path='db/migrations/20261018_member_access.sql';
const pr={url:'https://github.com/example/review-evidence/pull/42',base:'b'.repeat(40),head:'a'.repeat(40),baseRepo:'example/review-evidence',headRepo:'example/review-evidence',title:'Record access change',body:'This change removes member restrictions: all authenticated users can now read every record, regardless of ownership.',files:[
  {filename:'000-docs/format.md',status:'modified',patch:'@@ -1,0 +1,800 @@\n'+'+Documentation formatting change.\n'.repeat(1000)},
  {filename:path,status:'modified',patch:'@@ -1,4 +1,4 @@\n CREATE POLICY read_records ON public.records\n FOR SELECT TO authenticated\n-USING (true);\n+USING (member_id = auth.uid());\n'}
]};
const config={enabled:true,profile:'synthetic-grounding-probe',command:process.env.CLAUDE_COMMAND||'claude',model:'sonnet',autoPrepare:false,maxBudgetUsd:0.5};
const events=[];let c=new Companion(e=>events.push({...e,at:performance.now()}),{root});
try {
  await c.open({binding:'first',pr,config});
  const started=performance.now();
  await c.send({binding:'first',prompt:'Explain what this PR does in two short sentences. I want an explanation, not a review.'});await c.active.done;
  if(c.current.record.revision!==`${pr.base}:${pr.head}`)throw Error(events.findLast(e=>e.error)?.error||'Claude did not finish');
  const first=c.current.record.messages.at(-1).content,firstEvidence=c.current.evidence;
  if(!/(?:member_id|own|membership)/i.test(first))throw Error('Grounding probe did not identify the member restriction: '+first);
  if(!firstEvidence.providedPatchPaths.includes(path))throw Error('Migration was not supplied');
  const nativeSession=c.current.record.session;
  const firstDeltaMs=Math.round(events.find(e=>e.type==='delta').at-started);
  await c.close();
  c=new Companion(e=>events.push({...e,at:performance.now()}),{root});
  await c.open({binding:'resumed',pr,config});
  await c.send({binding:'resumed',prompt:'Which identity check enforces that behavior? Answer in one sentence.'});await c.active.done;
  if(c.current.record.messages.at(-1).role!=='assistant')throw Error(events.findLast(e=>e.error)?.error||'Resumed turn failed');
  const resumed=c.current.record.messages.at(-1).content;
  if(!/auth\.uid\(\)/i.test(resumed))throw Error('Resumed probe lost the implementation check: '+resumed);
  if(c.current.record.session!==nativeSession)throw Error('Native session changed');
  if(c.current.evidence.providedPatchPaths.includes(path)||!c.current.evidence.reusedPatchPaths.includes(path))throw Error('Completed migration evidence was resent instead of reused');
  console.log(JSON.stringify({firstAnswer:first,resumedAnswer:resumed,migrationPrioritized:firstEvidence.providedPatchPaths[0]===path,patchesSupplied:firstEvidence.providedPatchPaths.length,firstTurnPatchReads:firstEvidence.readPatchCount,firstTurnSourceReads:firstEvidence.readSourceCount,firstDeltaMs,nativeResume:true,migrationResent:false,successfulTurns:events.filter(e=>e.type==='turn_complete').length,perTurnBudgetUsd:0.5,githubWrites:0},null,2));
} finally {await c.close();await rm(root,{recursive:true,force:true});}
