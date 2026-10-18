import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile,chmod,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createCodexSession} from '../tui/src/ai-codex.mjs';
const ID='11111111-2222-4333-8444-555555555555';
async function fixture(t,mode='ok'){
 const cwd=await mkdtemp(join(tmpdir(),'rw-codex-'));t.after(()=>rm(cwd,{recursive:true,force:true}));
 const command=join(cwd,'codex.mjs');
 await writeFile(command,`#!/usr/bin/env node
import{createInterface}from'node:readline';import{appendFileSync,writeFileSync}from'node:fs';
writeFileSync('pid',String(process.pid));appendFileSync('events',JSON.stringify({args:process.argv.slice(2)})+'\\n');
const mode=${JSON.stringify(mode)},id=${JSON.stringify(ID)};
if(mode==='timeout'){process.on('SIGTERM',()=>{});setInterval(()=>{},1000);}
const send=x=>console.log(JSON.stringify(x));
createInterface({input:process.stdin}).on('line',line=>{
const m=JSON.parse(line);appendFileSync('events',line+'\\n');if(mode==='timeout'||!m.method)return;
if(m.method==='initialize'){
 send({id:m.id+100,result:{thread:{id:'stale'}}});send({method:'thread/started',params:{thread:{id:'stale'}}});
 send({id:m.id,result:{}});return;
}
if(m.method==='config/read'){send({id:m.id,result:{config:{mcp_servers:{inherited:{command:'unexpected'}},plugins:{inherited:{enabled:true}}}}});return;}
if(m.method==='thread/start'){
 if(mode==='error'){console.error('secret-error-details');send({id:m.id,error:{message:'secret-error-details'}});return;}
 if(mode==='oversize'){console.log('x'.repeat(2_100_000));return;}
 if(mode==='invalid'){send({id:m.id,result:{thread:{id:'not-a-session'}}});return;}
 if(mode==='exit'){process.exit(8);return;}
 send({id:900,method:'item/commandExecution/requestApproval',params:{}});
 send({id:m.id,result:{thread:{id}}});return;
}
if(m.method==='thread/name/set')send({id:m.id,result:{}});
}).on('close',()=>{if(mode!=='timeout')process.exit(0);});
`);await chmod(command,0o700);return {cwd,command};
}
async function assertExited(cwd){const pid=Number(await readFile(join(cwd,'pid'),'utf8'));assert.throws(()=>process.kill(pid,0),error=>error.code==='ESRCH');}
test('creates and names a public protocol session without inference, rejects approvals, ignores stale replies',async t=>{
 const opts=await fixture(t);const id=await createCodexSession({...opts,title:'LazyReview acme/repo #7',model:'default'});assert.equal(id,ID);
 const events=(await readFile(join(opts.cwd,'events'),'utf8')).trim().split('\n').map(JSON.parse);
 assert.deepEqual(events[0].args,['app-server','--stdio','-c','features.hooks=false']);
 const requests=events.filter(e=>e.method);
 assert.deepEqual(requests.map(e=>e.method),['initialize','initialized','config/read','thread/start','thread/name/set']);
 assert.equal(requests[3].params.ephemeral,false);assert.equal(requests[3].params.sandbox,'read-only');assert.ok(!('model'in requests[3].params));
 assert.deepEqual(requests[3].params.config,{mcp_servers:{inherited:{enabled:false}},plugins:{inherited:{enabled:false}},features:{hooks:false}});
 assert.deepEqual(requests[4].params,{threadId:ID,name:'LazyReview acme/repo #7'});
 assert.equal(events.find(e=>e.id===900).error.code,-32601);await assertExited(opts.cwd);
});
test('passes explicit model without pinning the local default',async t=>{
 const opts=await fixture(t);await createCodexSession({...opts,title:'LazyReview test',model:'chosen-model'});
 const events=(await readFile(join(opts.cwd,'events'),'utf8')).trim().split('\n').map(JSON.parse);
 assert.equal(events.find(e=>e.method==='thread/start').params.model,'chosen-model');
});
for(const mode of ['error','invalid','exit','oversize'])test(`rejects ${mode} responses and reaps the owned child`,async t=>{
 const opts=await fixture(t,mode);await assert.rejects(createCodexSession({...opts,title:'LazyReview test'}),error=>!error.message.includes('secret-error-details'));await assertExited(opts.cwd);
});
test('bounded timeout kills an unresponsive owned child',async t=>{
 const opts=await fixture(t,'timeout'),before=Date.now();await assert.rejects(createCodexSession({...opts,title:'LazyReview test',timeoutMs:100}),/timed out/);
 assert.ok(Date.now()-before<2500);await assertExited(opts.cwd);
});
test('missing executable returns a safe actionable error',async()=>{
 await assert.rejects(createCodexSession({command:'/no-such-rw-codex',cwd:tmpdir(),title:'LazyReview test'}),/configured executable/);
});
