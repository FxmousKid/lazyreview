import {createInterface} from 'node:readline';
import {sample} from '../lib/sample.ts';
import {categorize,parsePatch,splitRows} from '../lib/review.ts';
const item=(repo,n,title,state='MERGED')=>({repository:repo,number:n,title,author:'you',url:`https://github.com/${repo}/pull/${n}`,state,created:'2026-10-01T12:00:00Z'});
let historyCalls=0;
for await(const line of createInterface({input:process.stdin})) {
  const input=JSON.parse(line);let data;
  if(input.operation==='repositories')data={repositories:[{name:'acme/current',openPrs:1,commits:4,lastContribution:'2026-10-01'}],viewer:'you',total:1};
  else if(input.operation==='discover')data={items:[item('acme/current',9,'Current work','OPEN')],viewer:'you',total:1,page:1};
  else if(input.operation==='history') {
    if(input.repository)data={kind:'pulls',repository:input.repository,items:[item(input.repository,1234,'External author change','OPEN')]};
    else { if(historyCalls++>0)await new Promise(resolve=>setTimeout(resolve,800)); data={kind:'catalog',viewer:'you',repositories:[{name:'acme/current',prs:1,lastContribution:'2026-10-01'},{name:'archive/library',prs:2,lastContribution:'2024-03-03'}],items:[item('acme/current',9,'Current work','OPEN'),item('archive/library',42,'Fuzzy matching fixed'),item('archive/library',14,'Fix cache eviction','CLOSED')]}; }
  } else if(input.operation==='load') {
    data=structuredClone(sample);data.url=input.url;data.files=data.files.map(f=>({...f,category:categorize(f.filename),lines:parsePatch(f.patch),splitRows:splitRows(parsePatch(f.patch))}));
  } else if(input.operation==='syntax')data={left:null,right:null};
  else throw Error('Unexpected picker fixture operation: '+input.operation);
  console.log(JSON.stringify({ok:true,data}));
}
