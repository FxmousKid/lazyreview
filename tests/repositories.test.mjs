import test from 'node:test';
import assert from 'node:assert/strict';
import { discoverRepositories } from '../tui/src/repositories.mjs';
test('only repositories with authored open PRs are counted, deduplicated, and sorted by default-branch commits', async () => {
  const calls=[];
  const result=await discoverRepositories({},'token',async(url,init)=>{
    const body=JSON.parse(init.body);calls.push(body);
    assert.equal(url,'https://api.github.com/graphql');assert.equal(init.method,'POST');assert.ok(!body.query.includes('mutation'));
    if(body.query.includes('pullRequests')) return Response.json({data:{viewer:{id:'viewer-id',login:'me',pullRequests:{nodes:[{repository:{nameWithOwner:'a/low'}},{repository:{nameWithOwner:'a/high'}},{repository:{nameWithOwner:'a/low'}}],pageInfo:{hasNextPage:false}}}}});
    assert.equal(body.variables.author,'viewer-id');assert.match(body.query,/defaultBranchRef/);assert.match(body.query,/author:\{id:\$author\}/);
    assert.ok(!body.query.includes('unrelated'));
    return Response.json({data:{r0:{defaultBranchRef:{name:'main',target:{history:{totalCount:5}}}},r1:{defaultBranchRef:{name:'main',target:{history:{totalCount:90}}}}}});
  });
  assert.deepEqual(result.repositories.map(r=>[r.name,r.commits,r.openPrs]),[['a/high',90,1],['a/low',5,2]]);
  assert.equal(calls.length,2);assert.match(calls[0].query,/states:OPEN/);assert.equal(result.countBasis,'Your commits on the default branch');
});
test('authored PR pagination finds repos beyond first page and unknown counts sort last without becoming zero', async()=>{
  const result=await discoverRepositories({},'token',async(url,init)=>{
    const {query,variables}=JSON.parse(init.body);
    if(query.includes('pullRequests')) return Response.json({data:{viewer:{id:'v',login:'me',pullRequests:{nodes:[{repository:{nameWithOwner:variables.after?'a/known':'a/unknown'}}],pageInfo:variables.after?{hasNextPage:false}:{hasNextPage:true,endCursor:'next'}}}}});
    return Response.json({data:{r0:null,r1:{defaultBranchRef:{name:'main',target:{history:{totalCount:0}}}}},errors:[{path:['r0'],message:'private diagnostic'}]});
  });
  assert.deepEqual(result.repositories.map(r=>r.name),['a/known','a/unknown']);assert.equal(result.repositories[1].commits,null);assert.match(result.warning,/unavailable/);assert.ok(!JSON.stringify(result).includes('private diagnostic'));
});
test('repository search is independent of own PRs, supports public unauthenticated and exact private-access lookup', async()=>{
  const calls=[];
  const fetcher=async(url,init)=>{calls.push({url,init});return Response.json(url.includes('/search/')?{items:[{full_name:'someone/tool',description:'A tool'}],total_count:65,incomplete_results:true}:{full_name:'someone/tool',description:'A tool'});};
  let data=await discoverRepositories({query:'tool',page:2},'',fetcher);
  assert.equal(data.repositories[0].name,'someone/tool');assert.equal(data.hasNext,true);assert.equal(data.incomplete,true);assert.equal(calls[0].init.method,'GET');assert.equal(calls[0].init.headers.Authorization,undefined);assert.equal(new URL(calls[0].url).searchParams.get('page'),'2');
  data=await discoverRepositories({query:'someone/tool'},'token',fetcher);assert.equal(calls[1].url,'https://api.github.com/repos/someone/tool');assert.equal(data.total,1);
});
test('no authored open PRs means no commit-count queries',async()=>{
  let calls=0;
  const data=await discoverRepositories({},'token',async()=>{calls++;return Response.json({data:{viewer:{id:'v',login:'me',pullRequests:{nodes:[],pageInfo:{hasNextPage:false}}}}});});
  assert.deepEqual(data.repositories,[]);assert.equal(calls,1);
});
test('auth failures and rate limits leave an actionable sanitized error, count outage preserves repositories',async()=>{
  await assert.rejects(discoverRepositories({},'',()=>{throw Error('unreachable')}),/gh auth login/);
  await assert.rejects(discoverRepositories({query:'tool'},'',async()=>new Response('secret',{status:403})),e=>/HTTP 403/.test(e.message)&&!e.message.includes('secret'));
  const data=await discoverRepositories({},'token',async(url,init)=>JSON.parse(init.body).query.includes('pullRequests')?Response.json({data:{viewer:{id:'v',login:'me',pullRequests:{nodes:[{repository:{nameWithOwner:'a/b'}}],pageInfo:{hasNextPage:false}}}}}):new Response('secret',{status:429}));
  assert.equal(data.repositories[0].commits,null);assert.match(data.warning,/unavailable/);
});

test('recent authored contribution sorts before lifetime commit count',async()=>{
  const data=await discoverRepositories({},'token',async(url,init)=>{
    const {query}=JSON.parse(init.body);
    if(query.includes('pullRequests'))return Response.json({data:{viewer:{id:'v',login:'me',pullRequests:{nodes:[{createdAt:'2026-01-01',repository:{nameWithOwner:'a/old'}},{createdAt:'2026-09-01',repository:{nameWithOwner:'a/new'}}],pageInfo:{hasNextPage:false}}}}});
    return Response.json({data:{r0:{defaultBranchRef:{target:{history:{totalCount:100,nodes:[{committedDate:'2026-02-01'}]}}}},r1:{defaultBranchRef:{target:{history:{totalCount:1,nodes:[]}}}}}});
  });
  assert.deepEqual(data.repositories.map(r=>r.name),['a/new','a/old']);
});
