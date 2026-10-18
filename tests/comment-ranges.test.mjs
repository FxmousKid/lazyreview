import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
const source=(await readFile(new URL('../tui/src/comment.mjs',import.meta.url),'utf8')).replace("'./review.ts'",JSON.stringify(new URL('../lib/review.ts',import.meta.url).href));
const {prepareComment}=await import('data:text/javascript,'+encodeURIComponent(source));
const patch='@@ -1,2 +1,2 @@\n-old\n+new\n context\n@@ -8 +8 @@\n-other\n+replacement';
const pr={owner:'acme',repo:'atlas',headRepo:'fork/atlas',baseRepo:'acme/atlas',head:'head-sha',base:'merge-base',files:[{filename:'src/new name.ts',previous_filename:'src/old name.ts',patch}]};
const range=(side,start,hunk=0)=>({path:pr.files[0].filename,side,start,end:start,hunk});
test('separate ranges become one native anchor and immutable fork/base links',()=>{
  const out=prepareComment({body:'Compare these paths',ranges:[range('RIGHT',1),range('LEFT',1),range('RIGHT',8,1)]},pr);
  assert.equal(out.path,'src/new name.ts');assert.equal(out.side,'RIGHT');assert.equal(out.startLine,1);assert.equal(out.line,1);
  assert.match(out.body,/acme\/atlas\/blob\/merge-base\/src\/old%20name.ts#L1/);
  assert.match(out.body,/fork\/atlas\/blob\/head-sha\/src\/new%20name.ts#L8/);
  assert.equal((out.body.match(/Related code:/g)||[]).length,1);
});
test('one range stays a plain native line comment without added prose',()=>{
  const out=prepareComment({body:'Comment',ranges:[range('LEFT',1)]},pr);assert.equal(out.body,'Comment');assert.equal(out.side,'LEFT');
});
test('invalid, stale, cross-hunk and oversized reference selections reject before writes',()=>{
  for(const ranges of [[range('RIGHT',500)],[{...range('RIGHT',1),path:'not-in-pr'}],[{...range('RIGHT',1),end:8}],[range('OTHER',1)],Array.from({length:21},()=>range('RIGHT',1))]){
    assert.throws(()=>prepareComment({body:'Comment',ranges},pr));
  }
});
test('deleted-side related range resolves original path; added-side links remain on head',()=>{
  const deleted={...pr,files:[{...pr.files[0],status:'removed',patch:'@@ -1,2 +0,0 @@\n-old\n-more'}]};
  const out=prepareComment({body:'Deleted logic',ranges:[range('LEFT',1),range('LEFT',2)]},deleted);
  assert.match(out.body,/merge-base\/src\/old%20name.ts#L2/);
});
