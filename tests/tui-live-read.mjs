// Explicit authenticated read-only probe. Never logs repository names, code or credentials.
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {executeGithub} from '../lib/github.ts';
import {parsePatch} from '../lib/review.ts';
import {discoverRepositories} from '../tui/src/repositories.mjs';
import {discoverPullRequests} from '../tui/src/discovery.mjs';
import {complete} from '../tui/src/completion.mjs';
import {syntaxSources,readSnapshotContent} from '../tui/src/syntax.mjs';
if(!process.argv.includes('--live')) throw new Error('Pass --live and --pr URL (or --mine) to allow authenticated reads.');
const token=process.env.GH_TOKEN||process.env.GITHUB_TOKEN||execFileSync('gh',['auth','token','--hostname','github.com'],{encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim();
let url=process.argv[process.argv.indexOf('--pr')+1];
if(!process.argv.includes('--pr')) {
  if(!process.argv.includes('--mine')) throw new Error('Supply --pr URL or --mine.');
  const home=await discoverRepositories({},token);
  if(!home.repositories.length) throw new Error('No authored open PR repositories.');
  const list=await discoverPullRequests({scope:'repository',repository:home.repositories[0].name},token);
  if(!list.items.length) throw new Error('No open PRs in the selected repository.');
  url=list.items[0].url;
}
let requests=0;
const readOnly=async(endpoint,init={})=>{
  if(init.method&&init.method!=='GET') {
    const payload=JSON.parse(init.body||'{}');
    if(!String(payload.query||'').trim().startsWith('query')||String(payload.query).includes('mutation')) throw new Error('Write blocked in read-only probe.');
  }
  requests++;return fetch(endpoint,init);
};
const pr=await executeGithub({operation:'load',url},token,readOnly);
const candidates=[];
for(const file of pr.files){for(const line of parsePatch(file.patch)){const side=line.new?'RIGHT':line.old?'LEFT':null;if(side){const n=side==='RIGHT'?line.new:line.old;candidates.push({path:file.filename,side,start:n,end:n,hunk:line.hunk});break;}}if(candidates.length===2)break;}
if(candidates.length){
  const range=candidates[0];
  try{await executeGithub({operation:'content',url,path:range.path,side:range.side,expectedHead:pr.head,expectedBase:pr.base},token,readOnly);}catch(error){if(!String(error.message).includes('too large')&&!String(error.message).includes('binary'))throw error;}
}
let syntaxSides=0, syntaxLines=0;
const syntaxFile=pr.files.find(f=>f.filename===candidates[0]?.path);
if(syntaxFile){
  const sources=await syntaxSources({url,expectedHead:pr.head,expectedBase:pr.base,headRepo:pr.headRepo,baseRepo:pr.baseRepo,file:syntaxFile},null,request=>readSnapshotContent(request,token,readOnly));
  for(const [key,number] of [['left','old'],['right','new']]) {
    if(sources[key]===null)continue;
    syntaxSides++;
    const lines=sources[key].split('\n');
    for(const line of parsePatch(syntaxFile.patch))if(line[number]){
      if(lines[line[number]-1]?.replace(/\r$/,'')!==line.text.replace(/\r$/,''))throw Error('Immutable syntax source does not match diff');
      syntaxLines++;
    }
  }
}
const source=(await readFile(new URL('../tui/src/comment.mjs',import.meta.url),'utf8')).replace("'./review.ts'",JSON.stringify(new URL('../lib/review.ts',import.meta.url).href));
const {prepareComment}=await import('data:text/javascript,'+encodeURIComponent(source));
if(candidates.length)prepareComment({operation:'draft',body:'Read-only validation; never posted.',ranges:candidates},pr);
const users=await complete({url,kind:'user',query:'',id:1},token,readOnly);
const refs=await complete({url,kind:'issue',query:String(pr.number),id:2},token,readOnly);
console.log(JSON.stringify({authenticated:!!pr.viewer,files:pr.files.length,threads:pr.threads.length,commits:pr.commits.length,submittedReviews:(pr.reviews||[]).length,datedCommits:pr.commits.filter(c=>c.createdAt).length,datedReviews:(pr.reviews||[]).filter(r=>r.createdAt).length,missingPatches:pr.files.filter(f=>!f.patch).length,preparedRanges:candidates.length,mentionSuggestions:users.items.length,currentPrReference:refs.items.some(i=>i.text==='#'+pr.number),syntaxSides,syntaxLines,readRequests:requests,writes:0}));
