// Opt-in real Claude + public GitHub read probe. No GitHub mutation tools exist.
import {execFileSync} from 'node:child_process';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {randomUUID} from 'node:crypto';
import {executeGithub} from '../lib/github.ts';
import {Companion} from '../tui/src/ai-session.mjs';
if(!process.argv.includes('--live'))throw Error('Pass --live to use your Claude account for two bounded turns');
const token=process.env.GH_TOKEN||process.env.GITHUB_TOKEN||execFileSync('gh',['auth','token','--hostname','github.com'],{encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim();
const pr=await executeGithub({operation:'load',url:'https://github.com/primer/react/pull/8520'},token);
const root=await mkdtemp(join(tmpdir(),'rw-claude-live-'));
const events=[];const c=new Companion(e=>events.push({...e,at:performance.now()}),{root});
const config={enabled:true,profile:'validation',command:'claude',model:'sonnet',autoPrepare:false,maxBudgetUsd:0.5};
let session;
try{
 const marker='LazyReview-'+randomUUID().slice(0,8);
 await c.open({pr,config,binding:'1'});const started=performance.now();
 await c.send({binding:'1',prompt:`Remember this review marker: ${marker}. Call overview, then read_file on package.json lines 1 through 6 on the RIGHT. Reply in at most two sentences with the marker and what you observed.`,focus:{path:'package.json'},context:[]});
 await c.active?.done;
 const error=events.find(e=>e.error);if(error)throw Error(error.error);
 session=c.current.record.session;
 const text=c.current.record.messages.at(-1)?.content||'';if(!text.includes(marker))throw Error('First answer missing marker');
 const tools=events.find(e=>e.type==='session')?.tools||[];
 if(!tools.length||tools.some(t=>!t.startsWith('mcp__review__')))throw Error('Unexpected agent tool access: '+JSON.stringify(tools));
 const readCalls=events.filter(e=>e.type==='activity').length;if(readCalls<2)throw Error('Did not exercise repository tools');
 const firstDelta=events.find(e=>e.type==='delta');if(!firstDelta)throw Error('No streamed reply');
 await c.close();
 const resumed=new Companion(e=>events.push({...e,at:performance.now()}),{root});
 try {
  await resumed.open({pr,config,binding:'2'});
  if(resumed.current.record.session!==session)throw Error('Session changed');
  await resumed.send({binding:'2',prompt:'What was the review marker I asked you to remember? Reply only with that marker.',focus:{},context:[]});
  await resumed.active?.done;
  if(resumed.current.record.messages.at(-1)?.content.trim()!==marker)throw Error('Native memory was not resumed');
 }finally{await resumed.close();}
 console.log(JSON.stringify({nativeResume:true,streaming:true,availableReviewTools:tools.length,repositoryToolCalls:readCalls,firstDeltaMs:Math.round(firstDelta.at-started),turns:2,githubWrites:0}));
}finally{await c.close();await rm(root,{recursive:true,force:true});}
