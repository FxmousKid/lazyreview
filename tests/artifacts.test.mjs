import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm,readFile,writeFile,symlink,stat,unlink,mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawn} from 'node:child_process';
import {listArtifacts,readArtifact,createArtifact,openArtifact,viewer} from '../tui/src/artifacts.mjs';
import {callTool,tools} from '../tui/src/ai-tools.mjs';
import {Companion} from '../tui/src/ai-session.mjs';
const pr={url:'https://github.com/example/repo/pull/42',base:'b'.repeat(40),head:'a'.repeat(40),files:[],chatSession:'native-session'};
const args={kind:'svg',title:'Request flow',summary:'From UI through the handler.',content:'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 100"><text x="12" y="30">UI → handler</text></svg>'};
async function fixture(t){const dir=await mkdtemp(join(tmpdir(),'rw-artifact-test-'));t.after(()=>rm(dir,{recursive:true,force:true}));return dir;}
test('immutable artifacts round-trip, stay private and retain PR/session/revision provenance',async t=>{
 const dir=await fixture(t);assert.deepEqual(await listArtifacts(dir),[]);
 const one=await createArtifact(dir,pr,args);const two=await createArtifact(dir,{...pr,head:'c'.repeat(40)},{...args,title:'Revised flow'});
 assert.notEqual(one.id,two.id);assert.equal(one.session,'native-session');assert.equal(one.head,pr.head);
 const list=await listArtifacts(dir);assert.equal(list.length,2);assert.ok(list.every(v=>!('content' in v)));
 assert.equal((await readArtifact(dir,one.id)).content,args.content);
 assert.equal((await stat(join(dir,'artifacts',one.id,'source.json'))).mode&0o777,0o600);
 assert.equal((await stat(join(dir,'artifacts',one.id))).mode&0o777,0o700);
 let launched;await openArtifact(dir,one.id,p=>{launched=p;});assert.equal(launched,join(dir,'artifacts',one.id,'preview.html'));
});
test('path traversal, symlink storage, oversized content and tampered source fail closed',async t=>{
 const dir=await fixture(t);await assert.rejects(readArtifact(dir,'../../secret'),/Invalid artifact ID/);
 await assert.rejects(createArtifact(dir,pr,{...args,content:'x'.repeat(250001)}),/250 KB/);
 await assert.rejects(createArtifact(dir,pr,{...args,kind:'shell'}),/html, svg or markdown/);
 const item=await createArtifact(dir,pr,args),folder=join(dir,'artifacts',item.id);
 await writeFile(join(folder,'source.json'),JSON.stringify({content:'tampered'}));
 await assert.rejects(readArtifact(dir,item.id),/do not match/);
 await unlink(join(folder,'source.json'));await symlink(join(folder,'meta.json'),join(folder,'source.json'));
 await assert.rejects(readArtifact(dir,item.id));
 const other=await fixture(t);await symlink(join(dir,'artifacts'),join(other,'artifacts'));
 await assert.rejects(createArtifact(other,pr,args),/directory is unavailable/);
});
test('invalid/unfinished entries are absent; JSON escaping does not shrink the content allowance',async t=>{
 const dir=await fixture(t),item=await createArtifact(dir,pr,{...args,kind:'markdown',content:'"\\'.repeat(120000)});
 assert.equal((await readArtifact(dir,item.id)).content.length,240000);
 await writeFile(join(dir,'artifacts',item.id,'meta.json'),'{}');
 await mkdir(join(dir,'artifacts','00000000-0000-4000-8000-000000000000'));
 assert.deepEqual(await listArtifacts(dir),[]);
});
test('HTML preview escapes outer content and confines generated source to a static sandbox',async t=>{
 const dir=await fixture(t),source='<script>fetch("https://evil.test")</script><img src="https://evil.test"><h2>Evidence</h2>';
 const item=await createArtifact(dir,pr,{kind:'html',title:'</title><script>bad()</script>',summary:'A <b>summary</b>',content:source});
 const output=await readFile(join(dir,'artifacts',item.id,'preview.html'),'utf8');
 assert.ok(output.includes('sandbox=""'));assert.ok(output.includes("script-src 'none'"));assert.ok(output.includes('connect-src &amp;#39;none&amp;#39;'));
 assert.ok(!output.includes('<script>'));assert.ok(!output.includes('<img src="https://evil.test">'));
 assert.ok(output.includes('&lt;h2&gt;Evidence&lt;/h2&gt;'));assert.ok(!output.includes('allow-scripts'));
 await writeFile(join(dir,'artifacts',item.id,'preview.html'),'<script>bad()</script>');
 await openArtifact(dir,item.id,()=>{});assert.equal(await readFile(join(dir,'artifacts',item.id,'preview.html'),'utf8'),viewer({...item,content:source}));
});
test('only artifact_create writes, background preparation cannot create, and model paths are rejected',async t=>{
 const dir=await fixture(t);assert.deepEqual(tools.filter(t=>!t.annotations.readOnlyHint).map(t=>t.name),['artifact_create']);
 await assert.rejects(callTool(pr,'artifact_create',args,fetch,()=>'',dir),/disabled during background/);
 const value=await callTool({...pr,allowArtifacts:true},'artifact_create',args,fetch,()=>'',dir);
 assert.equal((await callTool(pr,'artifact_list',{},fetch,()=>'',dir)).items[0].id,value.id);
 assert.equal((await callTool(pr,'artifact_read',{id:value.id},fetch,()=>'',dir)).content,args.content);
 await assert.rejects(callTool({...pr,allowArtifacts:true},'artifact_create',{...args,path:'/tmp/elsewhere'},fetch,()=>'',dir),/Invalid tool arguments/);
});
test('stdio MCP saves an output and returns validation errors without exposing filesystem details',async t=>{
 const dir=await fixture(t),path=join(dir,'snapshot.json');await writeFile(path,JSON.stringify({...pr,allowArtifacts:true}));
 const child=spawn(process.execPath,[resolve('tui/src/ai-tools.mjs'),path],{stdio:['pipe','pipe','pipe']});let out='';child.stdout.on('data',v=>out+=v);child.stderr.resume();
 const done=new Promise((resolve,reject)=>{child.once('close',resolve);child.once('error',reject);});
 child.stdin.end([{id:1,args},{id:2,args:{...args,kind:'exe'}}].map(v=>JSON.stringify({jsonrpc:'2.0',id:v.id,method:'tools/call',params:{name:'artifact_create',arguments:v.args}})).join('\n')+'\n');
 assert.equal(await done,0);const results=out.trim().split('\n').map(JSON.parse);
 assert.ok(!results[0].result.isError);assert.equal(results[1].result.isError,true);assert.equal((await listArtifacts(dir)).length,1);
});
test('PR shelf survives session restart, new chat and new commits; background write gate follows turn type',{timeout:10000},async t=>{
 const root=await fixture(t),events=[];const config={enabled:true,profile:'artifact-test',command:resolve('tests/claude-fixture.mjs'),model:'fixture',autoPrepare:false,maxBudgetUsd:1};
 let accept;const accepted=new Promise(resolve=>{accept=resolve;});
 const c=new Companion(e=>{events.push(e);if(e.type==='turn_accepted')accept();},{root});t.after(()=>c.close());
 await c.open({pr,config,binding:'1'});const dir=c.current.dir;
 assert.equal(JSON.parse(await readFile(join(dir,'snapshot.json'),'utf8')).allowArtifacts,false);
 await c.send({binding:'1',prompt:'SIMULATE_SLOW'});await accepted;
 const snap=JSON.parse(await readFile(join(dir,'snapshot.json'),'utf8'));assert.equal(snap.allowArtifacts,true);assert.equal(snap.chatSession,c.current.record.session);
 const item=await createArtifact(dir,snap,args);await c.active?.done;
 assert.ok(events.some(e=>e.type==='artifacts'&&e.items.some(v=>v.id===item.id)));
 await c.fresh('1');assert.equal(c.current.artifacts.length,1);
 await c.open({pr:{...pr,head:'d'.repeat(40)},config,binding:'2'});assert.equal(c.current.artifacts[0].head,pr.head);
 c.run('',true);await c.active?.done;
 assert.equal(JSON.parse(await readFile(join(dir,'snapshot.json'),'utf8')).allowArtifacts,false);
});
