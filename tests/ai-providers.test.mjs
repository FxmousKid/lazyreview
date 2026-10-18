import test from 'node:test';import assert from 'node:assert/strict';
import {mkdtemp,rm,readFile,writeFile} from 'node:fs/promises';import {tmpdir} from 'node:os';import {join} from 'node:path';
import {PROVIDERS,provider,providerTable} from '../tui/src/ai-providers.mjs';
import {identity} from '../tui/src/ai-context.mjs';import {Companion} from '../tui/src/ai-session.mjs';import {hook} from '../tui/src/ai-native-hook.mjs';import {callTool} from '../tui/src/ai-tools.mjs';
const pr={url:'https://github.com/acme/repo/pull/7',owner:'acme',repo:'repo',number:7,title:'Guard access',head:'a'.repeat(40),base:'b'.repeat(40),body:'Check membership.',files:[{filename:'guard.ts',patch:'@@ -1 +1 @@\n-open()\n+checkMember()',status:'modified'}],threads:[],conversation:[]};
const config={enabled:true,provider:'grok',interface:'terminal',profile:'fixture',command:'grok',model:'default',autoPrepare:false,maxBudgetUsd:1};
async function fixture(t){const root=await mkdtemp(join(tmpdir(),'rw-provider-'));const events=[];const c=new Companion(e=>events.push(e),{root});t.after(async()=>{await c.close();await rm(root,{recursive:true,force:true});});await c.open({binding:'one',pr,config});await c.terminal('one');return {c,events,dir:c.current.dir,id:c.current.record.session};}
test('registry separates providers without changing legacy Claude associations',()=>{
 assert.equal(provider().id,'claude');assert.throws(()=>provider({provider:'shell'}));
 assert.equal(identity(pr,{profile:'default'}),identity(pr,{profile:'default',provider:'claude'}));
 assert.equal(new Set(Object.keys(PROVIDERS).map(provider=>identity(pr,{profile:'default',provider}))).size,3);
});
test('Grok launches direct argv with its own hooks/MCP and resumes the exact native session',async t=>{
 const {c,events,dir,id}=await fixture(t);let launch=events.find(e=>e.type==='terminal_launch');
 assert.equal(launch.command,'grok');assert.ok(launch.args.includes('--session-id'));assert.ok(!launch.args.includes('--model'));assert.ok(!launch.args.includes('--always-approve'));assert.ok(!launch.args.includes('--trust'));
 const hooks=JSON.parse(await readFile(join(dir,'.grok/hooks/review.json'),'utf8')).hooks;
 assert.ok(hooks.StopCancelled);assert.ok((await readFile(join(dir,'.grok/config.toml'),'utf8')).includes('[mcp_servers.review]'));
 const command=hooks.SessionStart[0].hooks[0].command;
 await hook({hook_event_name:'SessionStart',sessionId:id,source:'new'},dir,c.current.nativeLaunch);await c.pollNative();
 await c.close();await c.open({binding:'two',pr,config});await c.terminal('two');launch=events.filter(e=>e.type==='terminal_launch').at(-1);
 assert.ok(launch.args.includes('--resume'));assert.equal(launch.session,id);
 assert.equal(JSON.parse(await readFile(join(dir,'.grok/hooks/review.json'),'utf8')).hooks.SessionStart[0].hooks[0].command,command,'trust command stable across launches');
});
test('Grok notes clear only after context delivery and successful matching turn',async t=>{
 const {c,events,dir,id}=await fixture(t),launch=c.current.nativeLaunch;
 const item={id:'note',path:'guard.ts',side:'RIGHT',commit:pr.head,content:'checkMember()',note:'Who is allowed?'};
 await c.nativeContext({binding:'one',context:[item]});
 const ev=(kind,turn,extra={})=>({hook_event_name:kind,sessionId:id,promptId:turn,...extra});
 assert.deepEqual(await hook(ev('UserPromptSubmit','one',{prompt:'Explain'}),dir,launch),{},'Grok discards prompt hook context');
 await hook(ev('Stop','one',{lastAssistantMessage:'No tools read'}),dir,launch);await c.pollNative();
 assert.deepEqual(events.filter(e=>e.type==='native_complete').at(-1).items,[]);
 await hook(ev('UserPromptSubmit','two',{prompt:'Explain'}),dir,launch);
 const snapshot=JSON.parse(await readFile(join(dir,'snapshot.json'),'utf8'));
 const result=await callTool(snapshot,'review_context',{question:'Explain'},undefined,undefined,dir);
 assert.ok(result.context.includes('checkMember()'));assert.ok(result.context.includes('Who is allowed?'));
 await hook(ev('StopCancelled','one'),dir,launch); // delayed cancellation cannot cancel newer turn
 await hook(ev('Stop','one',{lastAssistantMessage:'stale answer'}),dir,launch);await c.pollNative();
 assert.equal(c.current.record.messages.length,2);
 await hook(ev('Stop','two',{lastAssistantMessage:'Members only.'}),dir,launch);await c.pollNative();
 assert.deepEqual(events.filter(e=>e.type==='native_complete').at(-1).items,[item]);assert.equal(c.current.record.messages.at(-1).content,'Members only.');
});
test('cancelled and undelivered turns retain their notes',async t=>{
 const {c,events,dir,id}=await fixture(t);await hook({hook_event_name:'UserPromptSubmit',sessionId:id,promptId:'x',prompt:'Explain'},dir);
 await hook({hook_event_name:'StopCancelled',sessionId:id,promptId:'x'},dir);
 await hook({hook_event_name:'Stop',sessionId:id,promptId:'x',lastAssistantMessage:'late'},dir);await c.pollNative();
 assert.ok(!events.some(e=>e.type==='native_complete'));
});
test('provider capability table stays synchronized with integration documentation',async()=>{
 const docs=await readFile(new URL('../tui/CLI-INTEGRATIONS.md',import.meta.url),'utf8');
 assert.ok(docs.includes(providerTable()),'Update the provider table whenever adapter capabilities change');
 for(const id of Object.keys(PROVIDERS))assert.ok(docs.includes('## '+id));
});
