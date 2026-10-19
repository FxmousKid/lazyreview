import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {existsSync} from 'node:fs';
import {TuiSessions,nativeBinary} from '../scripts/tui-sessions.mjs';
const binary=nativeBinary();
const available=!!binary&&existsSync(binary);
// npm test also runs before the native build; CI runs this file again after build.
test('hosted native sessions isolate state, reject outside origins and expire cleanly',{skip:!available},async()=>{
 const pool=new TuiSessions({binary,max:2,idleMs:300000});
 const server=createServer(async(req,res)=>{if(!await pool.handle(req,res)){res.writeHead(404);res.end();}});
 await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
 const base=`http://127.0.0.1:${server.address().port}`;
 const post=async body=>{const r=await fetch(base+'/lazyreview/demo/api',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(body)});return {status:r.status,data:await r.json()};};
 try{
  const a=await post({action:'create',cols:120,rows:34}),b=await post({action:'create',cols:120,rows:34});
  assert.equal(a.status,200);assert.equal(b.status,200);assert.notEqual(a.data.session,b.data.session);
  const id=a.data.session,other=b.data.session;
  let result=await post({session:id,events:[{type:'key',key:'2'}]});assert.equal(result.data.state.category,'Tests');
  result=await post({session:other,events:[]});assert.equal(result.data.state.category,'Code');
  result=await post({action:'create',cols:120,rows:34});assert.equal(result.status,400);assert.match(result.data.error,/busy/);
  result=await post({session:id,events:[{type:'shell',command:'echo should-never-run'}]});assert.equal(result.status,400);assert.match(result.data.error,/Unknown input/);
  result=await post({session:id,events:[{type:'resize',cols:9000,rows:1}]});assert.equal(result.status,400);
  const forbidden=await fetch(base+'/lazyreview/demo/api',{method:'POST',headers:{'content-type':'application/json',origin:'https://example.invalid'},body:JSON.stringify({session:id,events:[]})});assert.equal(forbidden.status,403);
  const huge=await fetch(base+'/lazyreview/demo/api',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({text:'x'.repeat(40000)})});assert.equal(huge.status,413);
  const worker=pool.sessions.get(id),pid=worker.child.pid,home=worker.home;
  await post({action:'close',session:id});assert.equal((await post({session:id,events:[]})).status,410);
  for(let i=0;i<100&&existsSync(home);i++)await new Promise(r=>setTimeout(r,20));
  assert.equal(existsSync(home),false,'private HOME removed on exit');assert.throws(()=>process.kill(pid,0),'native worker exited');assert.throws(()=>process.kill(-pid,0),'private bridge children exited');
  const session=pool.sessions.get(other);session.touched=0;pool.sweep();assert.equal(pool.sessions.size,0);
 }finally{pool.close();await new Promise(resolve=>server.close(resolve));}
});
