// Explicit read-only timing probe. Never logs repository names, source, or credentials.
import {execFileSync} from 'node:child_process';
import {executeGithub} from '../lib/github.ts';
import {syntaxSources,readSnapshotContent} from '../tui/src/syntax.mjs';
import {parsePatch} from '../lib/review.ts';
if(!process.argv.includes('--live') || !process.argv.includes('--pr')) throw Error('Use --live --pr URL');
const url=process.argv[process.argv.indexOf('--pr')+1];
const token=process.env.GH_TOKEN||process.env.GITHUB_TOKEN||execFileSync('gh',['auth','token','--hostname','github.com'],{encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim();
let requests=0;
const readOnly=async(endpoint,init={})=>{
  if(init.method && init.method!=='GET') {
    const payload=JSON.parse(init.body||'{}');
    if(!String(payload.query||'').trim().startsWith('query')||String(payload.query).includes('mutation')) throw Error('Writes forbidden');
  }
  requests++;return fetch(endpoint,init);
};
const pr=await executeGithub({operation:'load',url},token,readOnly);
const file=pr.files.find(f=>f.status==='modified' && f.patch);
if(!file)throw Error('No suitable source file');
const request={url,expectedHead:pr.head,expectedBase:pr.base,headRepo:pr.headRepo,baseRepo:pr.baseRepo,file};
const samples=[];
for(let run=0;run<3;run++){
  requests=0;
  const start=performance.now();
  const sources=await syntaxSources(request,null,r=>readSnapshotContent(r,token,readOnly));
  let matchedLines=0;
  for(const [key,number] of [['left','old'],['right','new']]){
    if(sources[key]==null)throw Error('Source unavailable');
    const lines=sources[key].split('\n');
    for(const line of parsePatch(file.patch))if(line[number]){
      if(lines[line[number]-1]?.replace(/\r$/,'')!==line.text.replace(/\r$/,''))throw Error('Wrong immutable source');
      matchedLines++;
    }
  }
  samples.push({ms:Math.round(performance.now()-start),requests,matchedLines});
}
console.log(JSON.stringify({samples,medianMs:samples.map(s=>s.ms).sort((a,b)=>a-b)[1],writes:0}));
