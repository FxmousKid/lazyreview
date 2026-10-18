import test from 'node:test';import assert from 'node:assert/strict';
import {mkdtemp,rm,readFile,writeFile} from 'node:fs/promises';import {tmpdir} from 'node:os';import {join} from 'node:path';
import {Companion} from '../tui/src/ai-session.mjs';import {hook} from '../tui/src/ai-native-hook.mjs';
const pr={url:'https://github.com/acme/repo/pull/7',owner:'acme',repo:'repo',number:7,title:'Guard access',head:'a'.repeat(40),base:'b'.repeat(40),body:'A changed guard.',files:[{filename:'guard.ts',patch:'@@ -1 +1 @@\n-open()\n+checkMember()',status:'modified'}],threads:[],conversation:[]};
const config={enabled:true,interface:'terminal',profile:'fixture',command:'claude',model:'sonnet',autoPrepare:false,maxBudgetUsd:1};
async function fixture(t){const root=await mkdtemp(join(tmpdir(),'rw-terminal-'));const events=[];const c=new Companion(e=>events.push(e),{root});t.after(async()=>{await c.close();await rm(root,{recursive:true,force:true});});await c.open({binding:'one',pr,config});return {c,events};}
test('terminal launch reuses per-PR session, names it, and never launches a print-mode child',async t=>{
 const {c,events}=await fixture(t);await c.terminal('one');let e=events.find(e=>e.type==='terminal_launch');
 assert.ok(e.args.includes('--session-id'));assert.ok(!e.args.includes('-p'));assert.ok(e.title.startsWith('LazyReview acme/repo #7'));const id=e.session;
 await hook({hook_event_name:'SessionStart',session_id:id},c.current.dir);await c.pollNative();
 await c.close();await c.open({binding:'two',pr,config});await c.terminal('two');e=events.filter(e=>e.type==='terminal_launch').at(-1);
 assert.equal(e.session,id);assert.ok(e.args.includes('--resume'));assert.ok(!e.args.includes('--dangerously-skip-permissions'));
 const old=id;await c.fresh('two');await c.terminal('two');assert.notEqual(c.current.record.session,old);
});
test('prompt hook injects fresh code and queued notes; only successful stop consumes exact items',async t=>{
 const {c,events}=await fixture(t);const dir=c.current.dir,id=c.current.record.session;
 const item={id:'deep/'.repeat(60),path:'guard.ts',side:'RIGHT',commit:pr.head,content:'1: checkMember()',note:'Who is allowed?'};
 await c.nativeContext({binding:'one',context:[item],focus:{scope:'pr'}});
 const result=await hook({hook_event_name:'UserPromptSubmit',session_id:id,prompt:'Explain the guard'},dir);
 assert.ok(result.hookSpecificOutput.additionalContext.includes('checkMember'));
 assert.ok(result.hookSpecificOutput.additionalContext.includes('Who is allowed'));
 await hook({hook_event_name:'StopFailure',session_id:id,error:'rate_limit'},dir);await c.pollNative();assert.ok(!events.some(e=>e.type==='native_complete'));
 await hook({hook_event_name:'UserPromptSubmit',session_id:id,prompt:'Try again'},dir);
 await hook({hook_event_name:'Stop',session_id:id,last_assistant_message:'Members only.'},dir);await c.pollNative();
 assert.deepEqual(events.find(e=>e.type==='native_complete').items,[item]);assert.equal(c.current.record.messages.at(-1).content,'Members only.');
 const again=await hook({hook_event_name:'UserPromptSubmit',session_id:id,prompt:'Recall the conclusion'},dir);
 assert.ok(!again.hookSpecificOutput.additionalContext.includes('@@ -1'));
 const before=events.length;await hook({hook_event_name:'Stop',session_id:'wrong',last_assistant_message:'wrong'},dir);await c.pollNative();assert.equal(events.length,before+1); // pending submit only
});
test('context is revision scoped and failed hook cannot pretend to supply notes',async t=>{
 const {c}=await fixture(t),id=c.current.record.session,dir=c.current.dir;
 await c.nativeContext({binding:'one',context:[{id:'stale',side:'RIGHT',commit:'c'.repeat(40),content:'stale'}]});
 const result=await hook({hook_event_name:'UserPromptSubmit',session_id:id,prompt:'Explain'},dir);assert.ok(!result.hookSpecificOutput.additionalContext.includes('"id":"stale"'));
 await writeFile(join(dir,'snapshot.json'),'broken');await assert.rejects(hook({hook_event_name:'UserPromptSubmit',session_id:id,prompt:'Explain'},dir));
});

test('new native chat at the same revision receives its own code context',async t=>{
 const {c,events}=await fixture(t),dir=c.current.dir,id=c.current.record.session;
 await c.nativeContext({binding:'one'});
 await hook({hook_event_name:'UserPromptSubmit',session_id:id,prompt:'Explain'},dir);
 await hook({hook_event_name:'Stop',session_id:id,last_assistant_message:'Done'},dir);await c.pollNative();
 await c.fresh('one');await c.nativeSnapshot();const next=c.current.record.session;
 await hook({hook_event_name:'Stop',session_id:next,last_assistant_message:'No turn yet'},dir);await c.pollNative();
 assert.equal(c.current.record.messages.length,0);
 const result=await hook({hook_event_name:'UserPromptSubmit',session_id:next,prompt:'Explain'},dir);
 assert.ok(result.hookSpecificOutput.additionalContext.includes('@@ -1'));
 assert.ok(result.hookSpecificOutput.additionalContext.includes('checkMember'));
});
test('native clear adopts its UUID, supplies fresh evidence, and excludes old launch hooks',async t=>{
 const {c,events}=await fixture(t);await c.terminal('one');const dir=c.current.dir,first=c.current.record.session,launch=c.current.nativeLaunch;
 await hook({hook_event_name:'UserPromptSubmit',session_id:first,prompt:'Explain'},dir,launch);
 await hook({hook_event_name:'Stop',session_id:first,last_assistant_message:'Done'},dir,launch);await c.pollNative();
 const next='11111111-2222-4333-8444-555555555555';
 await hook({hook_event_name:'SessionStart',source:'clear',session_id:next},dir,launch);
 // A context update before polling cannot overwrite the native session chosen by /clear.
 await c.nativeContext({binding:'one'});assert.equal(JSON.parse(await readFile(join(dir,'snapshot.json'),'utf8')).chatSession,next);
 const result=await hook({hook_event_name:'UserPromptSubmit',session_id:next,prompt:'Explain again'},dir,launch);
 assert.ok(result.hookSpecificOutput.additionalContext.includes('@@ -1'));
 await hook({hook_event_name:'Stop',session_id:next,last_assistant_message:'New context'},dir,launch);await c.pollNative();
 assert.equal(c.current.record.session,next);assert.equal(c.current.record.messages.length,2);
 assert.equal(JSON.parse(await readFile(join(dir,`archived-${first}.json`),'utf8')).messages.length,2);
 await c.terminal('one');const latest=events.filter(e=>e.type==='terminal_launch').at(-1);assert.ok(latest.args.includes('--resume'));assert.equal(latest.session,next);
 await hook({hook_event_name:'SessionStart',source:'clear',session_id:first},dir,launch);await c.pollNative();assert.equal(c.current.record.session,next);
});

test('closing or starting fresh drains an in-flight native Stop before replacing its session',async t=>{
 for(const action of ['close','fresh'])await t.test(action,async t=>{
  const {c,events}=await fixture(t);await c.terminal('one');clearInterval(c.nativePoll);
  const old=c.current,dir=old.dir,id=old.record.session,launch=old.nativeLaunch;
  await hook({hook_event_name:'UserPromptSubmit',session_id:id,prompt:'Old PR turn'},dir,launch);
  await hook({hook_event_name:'Stop',session_id:id,last_assistant_message:'Old answer'},dir,launch);
  let unblock,notify;const gate=new Promise(resolve=>{unblock=resolve;}),blocked=new Promise(resolve=>{notify=resolve;});
  const save=c.save.bind(c);let once=false;
  c.save=async()=>{if(c.current===old&&old.record.nativeLastTurn&&!once){once=true;notify();await gate;}return save();};
  const poll=c.pollNative();await blocked;
  let finished=false;const transition=(action==='close'?c.close():c.fresh('one')).then(()=>{finished=true;});
  try{
   await new Promise(resolve=>setImmediate(resolve));
   assert.equal(finished,false,'The transition must wait for the active receipt poll');
   assert.equal(c.current.record.session,id);
  }finally{unblock();}
  await Promise.all([poll,transition]);
  if(action==='close')await c.open({binding:'two',pr:{...pr,number:8,url:'https://github.com/acme/repo/pull/8'},config});
  else assert.notEqual(c.current.record.session,id);
  assert.equal(events.filter(e=>e.type==='native_complete').length,1);
  assert.equal(events.find(e=>e.type==='native_complete').binding,'one');
  assert.ok(!events.some(e=>e.binding==='two'&&['native_complete','native_status'].includes(e.type)));
  if(action==='fresh')assert.equal(JSON.parse(await readFile(join(dir,`archived-${id}.json`),'utf8')).messages.at(-1).content,'Old answer');
 });
});
