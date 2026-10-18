import test from 'node:test';
import assert from 'node:assert/strict';
import {syntaxSources,readSnapshotContent} from '../tui/src/syntax.mjs';
import {sample} from '../lib/sample.ts';
const input = {url:'https://github.com/a/b/pull/1',expectedHead:'a'.repeat(40),expectedBase:'b'.repeat(40),headRepo:'fork/repo',baseRepo:'base/repo',file:{filename:'new.ts',previous_filename:'old.js',status:'renamed'}};
test('syntax reads both immutable sides, with the original path retained for grammar detection', async () => {
  const calls=[];
  const data=await syntaxSources(input,sample,async req=>{calls.push(req);return {commit:req.commit,content:req.side==='LEFT'?'/* before */':'// after'};});
  assert.equal(calls.length,2);
  for(const request of calls){const left=request.side==='LEFT';assert.equal(request.path,left?'old.js':'new.ts');assert.equal(request.repository,left?'base/repo':'fork/repo');assert.equal(request.commit,left?input.expectedBase:input.expectedHead);}
  assert.deepEqual(data,{left:'/* before */',right:'// after',leftPath:'old.js',rightPath:'new.ts'});
});
test('missing new/deleted sides are empty and never requested',async()=>{
  for(const [status,side] of [['added','RIGHT'],['removed','LEFT']]) {
    const calls=[];
    const data=await syntaxSources({...input,file:{...input.file,status}},sample,async r=>{calls.push(r.side);return {commit:r.commit,content:'source'};});
    assert.deepEqual(calls,[side]);assert.equal(data[side==='RIGHT'?'left':'right'],'');
  }
});
test('partial failure and wrong snapshot never substitute colors from another revision',async()=>{
  const partial=await syntaxSources(input,sample,async r=>{if(r.side==='LEFT')throw Error('large/binary/stale');return {commit:r.commit,content:'right'};});
  assert.equal(partial.left,null);assert.equal(partial.right,'right');
  const mismatch=await syntaxSources(input,sample,async()=>({commit:'new-head',content:'stale'}));
  assert.equal(mismatch.left,null);assert.equal(mismatch.right,null);
});
test('sample syntax has complete original and modified files and never authenticates or reads GitHub',async()=>{
  const data=await syntaxSources({...input,demo:true,expectedHead:sample.head,expectedBase:sample.base,file:sample.files[0]},sample,()=>{throw Error('Network forbidden');});
  assert.equal(data.left,sample.files[0].originalContent);assert.equal(data.right,sample.files[0].content);
  assert.ok(data.left.startsWith('import'));assert.ok(data.right.includes('useKeyboardNavigation'));
});

const payload = text => ({type:'file',encoding:'base64',size:Buffer.byteLength(text),content:Buffer.from(text).toString('base64')});
test('exactly two parallel immutable reads, including fork, original rename path and auth',async()=>{
  const calls=[];let release;
  const gate=new Promise(resolve=>release=resolve);
  const fetcher=async(url,options)=>{calls.push({url,options});await gate;return Response.json(payload('source'));};
  const pending=syntaxSources(input,sample,r=>readSnapshotContent(r,'fixture-token',fetcher));
  await new Promise(setImmediate);
  assert.equal(calls.length,2); // neither response has finished; no PR/files/compare requests
  assert.equal(calls[0].url,`https://api.github.com/repos/base/repo/contents/old.js?ref=${input.expectedBase}`);
  assert.equal(calls[1].url,`https://api.github.com/repos/fork/repo/contents/new.ts?ref=${input.expectedHead}`);
  for(const {options} of calls){assert.equal(options.method,'GET');assert.equal(options.headers.Authorization,'Bearer fixture-token');assert.equal(options.redirect,'error');}
  release();assert.equal((await pending).right,'source');
});
test('reader requires an immutable ref and safe repository/path before networking',async()=>{
  const request={repository:'base/repo',commit:input.expectedHead,path:'src/a.ts'};
  for(const invalid of [{commit:'main'},{repository:'https://elsewhere.test/repo'},{repository:'base/..'},{path:'../secret'},{path:'/absolute'},{path:'src/../secret'}]) {
    await assert.rejects(readSnapshotContent({...request,...invalid},'',()=>{assert.fail('Must not fetch');}));
  }
  const result=await readSnapshotContent({...request,path:'src/a #é.ts'},'',async(url,options)=>{
    assert.ok(url.includes('src/a%20%23%C3%A9.ts?ref='));assert.equal(options.headers.Authorization,undefined);return Response.json(payload('const café = 1;'));
  });
  assert.equal(result.content,'const café = 1;');
});
test('large, binary, invalid UTF-8, malformed, unavailable sources retain plain fallback',async()=>{
  const request={repository:'base/repo',commit:input.expectedHead,path:'src/a.ts'};
  for(const data of [{...payload('x'),size:200001},payload('a\0b'),{type:'file',encoding:'base64',size:1,content:'/w=='},{...payload('x'),size:2},{type:'dir'}, {...payload('x'),encoding:'none'}]) {
    await assert.rejects(readSnapshotContent(request,'',async()=>Response.json(data)));
  }
  await assert.rejects(readSnapshotContent(request,'',async()=>new Response('',{status:404})));
  assert.equal((await readSnapshotContent(request,'',async()=>Response.json(payload('')))).content,'');
});
