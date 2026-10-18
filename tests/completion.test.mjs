import test from 'node:test';
import assert from 'node:assert/strict';
import {complete} from '../tui/src/completion.mjs';
const url='https://github.com/acme/atlas/pull/128';
test('mention suggestions use repository-scoped GraphQL with variables and sanitized entries',async()=>{
  let request;
  const result=await complete({url,kind:'user',query:'ali',id:7},'token',async(endpoint,init)=>{request={endpoint,...JSON.parse(init.body)};return Response.json({data:{repository:{mentionableUsers:{nodes:[{login:'alice',name:'Alice'},{login:'bad user',name:'bad'}]}}}});});
  assert.equal(request.endpoint,'https://api.github.com/graphql');assert.deepEqual(request.variables,{owner:'acme',repo:'atlas',query:'ali'});assert.ok(!request.query.includes('mutation'));
  assert.deepEqual(result.items,[{text:'@alice',label:'Alice',kind:'user'}]);assert.equal(result.id,7);
});
test('issue and PR suggestions support recent, numeric and title search, without escaping repo',async()=>{
  const calls=[];const fetcher=async(endpoint,init)=>{calls.push({endpoint,init});const issue={number:42,title:'Example',pull_request:{}};return Response.json(endpoint.includes('/search/')?{items:[issue]}:endpoint.endsWith('/42')?issue:[issue]);};
  for(const query of ['','42','fix repo:evil/escape']){const data=await complete({url,kind:'issue',query},'',fetcher);assert.equal(data.items[0].text,'#42');assert.equal(data.items[0].kind,'PR');}
  assert.ok(calls[0].endpoint.includes('/repos/acme/atlas/issues?'));assert.ok(calls[1].endpoint.endsWith('/repos/acme/atlas/issues/42'));
  const q=new URL(calls[2].endpoint).searchParams.get('q');assert.ok(q.startsWith('repo:acme/atlas in:title '));assert.ok(!q.includes('repo:evil'));
  assert.ok(calls.every(c=>c.init.method==='GET'));
});
test('suggestion failures do not expose provider errors or require auth for references',async()=>{
  await assert.rejects(complete({url,kind:'issue',query:'42'},'',async()=>new Response('secret',{status:403})),e=>e.message==='Suggestions unavailable; continue typing.');
  assert.deepEqual((await complete({url,kind:'user',query:''},'',()=>{throw Error('unreachable')})).items,[]);
  await assert.rejects(complete({url:'https://attacker.example/a',kind:'user'},'token',()=>{throw Error('unreachable')}),/GitHub PR/);
});
