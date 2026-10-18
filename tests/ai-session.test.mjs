import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,readFile,rm,stat,symlink} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {Companion,acquireLock,cliArgs} from '../tui/src/ai-session.mjs';
import {identity,snapshot,briefing,questionContext} from '../tui/src/ai-context.mjs';
import {callTool,tools} from '../tui/src/ai-tools.mjs';
const pr={url:'https://github.com/base/repo/pull/42',base:'b'.repeat(40),head:'a'.repeat(40),baseRepo:'base/repo',headRepo:'fork/repo',title:'Change',body:'Description',files:[{filename:'new.ts',previous_filename:'old.ts',status:'renamed',patch:'@@ -1 +1 @@\n-old\n+new',lines:[{huge:true}]}],conversation:[{body:'Discussion'}],threads:[],commits:[]};
const cfg={enabled:true,profile:'test',command:resolve('tests/claude-fixture.mjs'),model:'fixture',autoPrepare:false,maxBudgetUsd:1};
async function fixture(t){const root=await mkdtemp(join(tmpdir(),'rw-ai-test-'));const events=[];const c=new Companion(e=>events.push(e),{root});t.after(async()=>{await c.close();await rm(root,{recursive:true,force:true});});return {c,events,root};}
const settle=async c=>{if(c.active)await c.active.done;};
const wait=async condition=>{for(let n=0;n<200;n++){if(condition())return;await new Promise(r=>setTimeout(r,10));}throw Error('Fixture timed out');};
test('LazyReview reuses legacy PR sessions and locks without moving native workspaces',async t=>{
  const temp=await mkdtemp(join(tmpdir(),'lazyreview-migration-'));
  const root=join(temp,'lazyreview'),legacyRoot=join(temp,'review-workbench');
  const old=new Companion(()=>{},{root:legacyRoot});
  const next=new Companion(()=>{},{root,legacyRoot});
  t.after(async()=>{await old.close();await next.close();await rm(temp,{recursive:true,force:true});});
  await old.open({pr,config:cfg,binding:'before'});
  const id=old.current.record.session,dir=old.current.dir;
  await assert.rejects(next.open({pr,config:cfg,binding:'locked'}),/already open/);
  await old.close();
  await next.open({pr,config:cfg,binding:'after'});
  assert.equal(next.current.record.session,id);assert.equal(next.current.dir,dir);
  await next.open({pr:{...pr,url:'https://github.com/base/repo/pull/43'},config:cfg,binding:'new'});
  assert.ok(next.current.dir.startsWith(root+'/'));
  assert.equal(JSON.parse(await readFile(join(dir,'session.json'),'utf8')).session,id);
});
test('PR identity survives revisions and key/model rotation, but isolates profiles and repositories',()=>{
  assert.equal(identity(pr,cfg),identity({...pr,head:'c'.repeat(40)},{...cfg,model:'another'}));
  assert.notEqual(identity(pr,cfg),identity(pr,{...cfg,profile:'other-account'}));
  assert.notEqual(identity(pr,cfg),identity({...pr,url:'https://github.com/other/repo/pull/42'},cfg));
  assert.throws(()=>identity({...pr,url:'https://evil.test/base/repo/pull/42'},cfg));
  assert.equal(snapshot(pr).files[0].lines,undefined);
});
test('native session resumes after restart, streams before completion, and display history is not resent',async t=>{
  const {c,events,root}=await fixture(t);
  await c.open({pr,config:cfg,binding:'1'});assert.equal(c.active,null);
  const id=c.current.record.session;
  await c.send({binding:'1',prompt:'First question',focus:{scope:'file',path:'new.ts'},context:[]});
  await wait(()=>events.some(e=>e.type==='delta'));assert.ok(c.active);
  await settle(c);assert.equal(c.current.record.revision,`${pr.base}:${pr.head}`);
  const dir=c.current.dir;assert.equal((await stat(join(dir,'session.json'))).mode&0o777,0o600);
  await c.close();
  const next=new Companion(e=>events.push(e),{root});
  try{
    await next.open({pr,config:{...cfg,autoPrepare:true},binding:'2'});
    assert.equal(next.current.record.session,id);assert.equal(next.active,null);assert.equal(next.current.record.messages.length,2);
    await next.send({binding:'2',prompt:'Second question',focus:{scope:'file',path:'README.md'}});await settle(next);
    const calls=JSON.parse(await readFile(join(dir,'fixture.json'),'utf8')).requests;
    assert.equal(calls[1].resume,true);assert.equal(calls[1].id,id);
    assert.ok(!calls[1].prompt.includes('First question'));assert.ok(!calls[1].prompt.includes('Description'));
    assert.ok(calls[1].prompt.includes('README.md'));
  }finally{await next.close();}
});
test('new commits refresh the same native session and new chat archives the old association',async t=>{
  const {c}=await fixture(t);await c.open({pr,config:cfg,binding:'1'});
  await c.send({binding:'1',prompt:'Question'});await settle(c);const id=c.current.record.session;
  const updated={...pr,head:'c'.repeat(40)};
  await c.open({pr:updated,config:{...cfg,autoPrepare:true},binding:'2'});await settle(c);
  assert.equal(c.current.record.session,id);assert.equal(c.current.record.revision,`${pr.base}:${updated.head}`);
  const calls=JSON.parse(await readFile(join(c.current.dir,'fixture.json'),'utf8')).requests;
  assert.ok(calls.at(-1).prompt.includes(`Previous comparison: ${pr.base}:${pr.head}`));
  await c.fresh('2');await settle(c);assert.notEqual(c.current.record.session,id);
  assert.equal(JSON.parse(await readFile(join(c.current.dir,`archived-${id}.json`),'utf8')).session,id);
});
test('first question interrupts preparation, cancellation and missing executable remain recoverable',async t=>{
  const {c,events}=await fixture(t);await c.open({pr,config:{...cfg,autoPrepare:true},binding:'1'});
  await wait(()=>c.current.record.started);
  await c.send({binding:'1',prompt:'Human priority'});await settle(c);
  assert.equal(c.current.record.messages[0].content,'Human priority');assert.equal(c.current.record.messages.length,2);
  await c.send({binding:'stale',prompt:'Never send'});assert.equal(c.active,null);
  await c.send({binding:'1',prompt:'SIMULATE_SLOW'});await c.stop();assert.equal(c.active,null);
  await c.open({pr,config:{...cfg,command:'/missing/claude'},binding:'2'});
  await c.send({binding:'2',prompt:'Check'});await settle(c);
  assert.ok(events.some(e=>e.error?.includes('not found')));
});
test('failed turns never mark a new revision prepared; concurrent lazyreviewes cannot own the same chat',async t=>{
  const {c}=await fixture(t);await c.open({pr,config:cfg,binding:'1'});
  await assert.rejects(acquireLock(c.current.dir),/already open/);
  await c.send({binding:'1',prompt:'SIMULATE_ERROR'});await settle(c);assert.equal(c.current.record.revision,null);
  const args=cliArgs(cfg,c.current.record,'mcp.json',false);
  assert.equal(args[args.indexOf('--tools')+1],'');assert.ok(args.includes('--strict-mcp-config'));assert.ok(!args.includes('--dangerously-skip-permissions'));
  assert.equal(args[args.indexOf('--permission-mode')+1],'dontAsk');
});
test('context is bounded and stale attachments are omitted; complete patches stay available through tools',async()=>{
  const large={...pr,files:[...pr.files,{filename:'large.ts',patch:'x'.repeat(100000)}]};
  const b=briefing(large);assert.ok(b.length<40000);assert.ok(b.includes('omittedPatches'));
  assert.ok(!questionContext(pr,{},[{content:'stale',commit:'wrong'},{content:'valid',commit:pr.head,side:'RIGHT'}]).includes('stale'));
  const patch=await callTool(large,'changes',{path:'large.ts'});assert.equal(patch.patch.length,100000);
});
test('repository tools only read pinned commits and expose no mutation tools',async()=>{
  assert.ok(tools.filter(t=>!t.name.startsWith('artifact_')).every(t=>t.annotations.readOnlyHint));
  const calls=[];const content='first\nsecond\nthird';
  const fetcher=async(url,options)=>{calls.push({url,options});return Response.json({type:'file',encoding:'base64',size:Buffer.byteLength(content),content:Buffer.from(content).toString('base64')});};
  const data=await callTool(pr,'read_file',{path:'old.ts',side:'LEFT',start:2,end:3},fetcher,()=> 'fixture');
  assert.equal(data.text,'2: second\n3: third');assert.equal(data.commit,pr.base);assert.ok(calls[0].url.includes(`/base/repo/contents/old.ts?ref=${pr.base}`));assert.equal(calls[0].options.method,'GET');
  await assert.rejects(callTool(pr,'read_file',{path:'../secret'},fetcher));assert.equal(calls.length,1);
  await assert.rejects(callTool(pr,'read_file',{path:'a',repository:'evil/repo'},fetcher));
  await assert.rejects(callTool(pr,'submit_review',{},fetcher));
  const tree=await callTool(pr,'tree',{query:'src'},async(url,options)=>{assert.equal(options.method,'GET');assert.ok(url.includes(`/fork/repo/git/trees/${pr.head}`));return Response.json({tree:[{type:'blob',path:'src/a.ts'},{type:'blob',path:'README.md'}],truncated:true});},()=> '');
  assert.deepEqual(tree.items,['src/a.ts']);assert.equal(tree.truncated,true);
});

test('stopping a turn also removes its own descendant processes',async t=>{
  const {c}=await fixture(t);await c.open({pr,config:cfg,binding:'1'});
  await c.send({binding:'1',prompt:'SIMULATE_CHILD'});
  let pid;
  for(let n=0;n<200;n++){try{pid=JSON.parse(await readFile(join(c.current.dir,'fixture-child.json'),'utf8')).pid;break;}catch{await new Promise(r=>setTimeout(r,10));}}
  assert.ok(pid);await c.close();
  await wait(()=>{try{process.kill(pid,0);return false;}catch(e){return e.code==='ESRCH';}});
});

test('embedded entrypoints run through macOS-style symlinked temporary paths',async t=>{
  const root=await mkdtemp(join(tmpdir(),'rw-ai-entry-'));t.after(()=>rm(root,{recursive:true,force:true}));
  for(const name of ['ai-session.mjs','ai-tools.mjs']) {
    const link=join(root,name);await symlink(resolve('tui/src/'+name),link);
    const child=spawn(process.execPath,[link],{stdio:['pipe','pipe','pipe']});
    let output='';child.stdout.on('data',data=>output+=data);child.stderr.resume();
    const done=new Promise((resolve,reject)=>{child.on('error',reject);child.on('close',resolve);});
    child.stdin.end(name==='ai-tools.mjs'?JSON.stringify({jsonrpc:'2.0',id:1,method:'initialize',params:{protocolVersion:'2025-11-25'}})+'\n':'invalid-json\n');
    assert.equal(await done,0);
    const event=JSON.parse(output.trim());
    if(name==='ai-tools.mjs')assert.equal(event.result.serverInfo.name,'lazyreview');else assert.equal(event.type,'error');
  }
});

test('PR-wide scope discards implicit focus but keeps explicit current attachments',()=>{
 const context=questionContext(pr,{path:'unselected.ts'},[{content:'explicit-code',commit:pr.head,side:'RIGHT'}]);
 assert.ok(context.includes('whole pull request'));assert.ok(!context.includes('unselected.ts'));assert.ok(context.includes('explicit-code'));
 assert.ok(questionContext(pr,{scope:'file',path:'selected.ts'}).includes('selected.ts'));
});
