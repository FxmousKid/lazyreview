import test from 'node:test';
import assert from 'node:assert/strict';
import { discoverHistory } from '../tui/src/repositories.mjs';
const pr=(number,repo='a/old',date='2020-01-01T00:00:00Z',state='MERGED')=>({number,title:`Improve feature ${number}`,url:`https://github.com/${repo}/pull/${number}`,repository:{nameWithOwner:repo},author:{login:'me'},createdAt:date,updatedAt:date,state,isDraft:false});
const page=(nodes,next=null)=>({nodes,pageInfo:{hasNextPage:!!next,endCursor:next}});
test('history follows every authored page, keeps merged PRs and unions paginated contributions',async()=>{
  const calls=[];
  const result=await discoverHistory({},'token',async(url,init)=>{
    const {query,variables}=JSON.parse(init.body);calls.push({query,variables});assert.ok(!query.includes('mutation'));
    if(query.includes('pullRequests')) return Response.json({data:{viewer:{id:'v',login:'me',pullRequests:variables.after?page([pr(2,'a/new','2026-01-01T00:00:00Z','OPEN')]):page([pr(1)],'second')}}});
    if(query.includes('repositoriesContributedTo')) return Response.json({data:{viewer:{repositoriesContributedTo:variables.after?page([{nameWithOwner:'a/commit-only',description:''}]):page([{nameWithOwner:'a/old',description:''}],'more')}}});
    return Response.json({data:{r0:{defaultBranchRef:{target:{history:page([])}}},r1:{defaultBranchRef:{target:{history:page([])}}},r2:{defaultBranchRef:{target:{history:page([{committedDate:'2026-02-01T00:00:00Z'}])}}}}});
  });
  assert.deepEqual(result.repositories.map(r=>r.name),['a/commit-only','a/new','a/old']);
  assert.equal(result.items.length,2);assert.equal(result.items[1].state,'MERGED');
  assert.equal(calls.filter(c=>c.query.includes('pullRequests')).length,2);
  assert.equal(calls.filter(c=>c.query.includes('repositoriesContributedTo')).length,2);
  assert.ok(calls.every(c=>!c.query.includes('states:OPEN')));
});
test('history is not capped at 1000 and repeated cursors fail instead of silently truncating',async()=>{
  let pages=0;
  const result=await discoverHistory({},'token',async(url,init)=>{
    const {query}=JSON.parse(init.body);
    if(query.includes('pullRequests')) { const offset=pages++*100;return Response.json({data:{viewer:{id:'v',login:'me',pullRequests:page(Array.from({length:100},(_,i)=>pr(offset+i+1)),pages<11?String(pages):null)}}}); }
    if(query.includes('repositoriesContributedTo')) return Response.json({data:{viewer:{repositoriesContributedTo:page([])}}});
    return Response.json({data:{r0:{defaultBranchRef:null}}});
  });
  assert.equal(result.items.length,1100);assert.equal(pages,11);
  await assert.rejects(discoverHistory({},'token',async()=>Response.json({data:{viewer:{id:'v',login:'me',pullRequests:page([pr(1)],'same')}}})),/pagination/);
});
test('exact repository retrieves all authors and all states without GitHub search limits',async()=>{
  let count=0;
  const result=await discoverHistory({repository:'someone/project'},'',async(url,init)=>{
    assert.equal(init.method,'GET');assert.equal(new URL(url).searchParams.get('state'),'all');
    count++;return Response.json(count===1?Array.from({length:100},(_,i)=>({number:i+1,title:'Change',html_url:`https://github.com/someone/project/pull/${i+1}`,user:{login:'someone'},created_at:'2026-01-01',state:'closed',merged_at:'2026-01-02'})):[]);
  });
  assert.equal(count,2);assert.equal(result.items.length,100);assert.equal(result.items[0].state,'MERGED');assert.equal(result.kind,'pulls');
  await assert.rejects(discoverHistory({repository:'a/b?x'},'token',()=>{throw Error('not reached')}),/owner\/repo/);
});
test('contribution lookup failure preserves full PR history with an explicit warning',async()=>{
  const result=await discoverHistory({},'token',async(url,init)=>{
    const {query}=JSON.parse(init.body);
    if(query.includes('pullRequests')) return Response.json({data:{viewer:{id:'v',login:'me',pullRequests:page([pr(1)])}}});
    return new Response('secret diagnostic',{status:403});
  });
  assert.equal(result.items.length,1);assert.match(result.warning,/contribution/i);assert.ok(!JSON.stringify(result).includes('secret diagnostic'));
});
