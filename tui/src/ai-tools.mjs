import {realpathSync} from 'node:fs';
// GitHub reads and bounded PR-local artifacts. All repository/commit authority comes from the captured PR.
import {readFile,writeFile,rename,unlink} from 'node:fs/promises';
import {randomUUID} from 'node:crypto';
import {createInterface} from 'node:readline';
import {execFileSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';
import {dirname,join} from 'node:path';
import {listArtifacts,readArtifact,createArtifact} from './artifacts.mjs';
import {readSnapshotContent} from './syntax.mjs';
import {briefing,patchEvidence,questionContext,currentAttachments} from './ai-context.mjs';
const string={type:'string'};
const integer={type:'integer',minimum:0};
const schema=properties=>({type:'object',properties,additionalProperties:false});
export const tools = [
  {name:'review_context',description:'Call before each reviewer answer. Supplies current PR revisions, bounded implementation patches, and queued private code questions. The question is used to prioritize relevant code. This does not post GitHub comments.',inputSchema:schema({question:string})},
  {name:'overview',description:'PR description, exact revisions, repository information and current reviewer focus.',inputSchema:schema({})},
  {name:'changes',description:'List changed files or read one complete returned patch. Paths use current names, including renames.',inputSchema:schema({path:string,offset:integer})},
  {name:'read_file',description:'Read surrounding repository code at the captured LEFT (merge base) or RIGHT (head) commit. Use original path for LEFT of renamed files. Returns numbered lines, at most 300 per call.',inputSchema:schema({path:string,side:{enum:['LEFT','RIGHT']},start:{type:'integer',minimum:1},end:{type:'integer',minimum:1}})},
  {name:'tree',description:'Find repository paths at the exact commit. Optional literal path substring; paginated. Reports GitHub tree truncation.',inputSchema:schema({query:string,side:{enum:['LEFT','RIGHT']},offset:integer})},
  {name:'search_changes',description:'Literal text search in loaded patches, with file paths. Use read_file to inspect surrounding code.',inputSchema:schema({query:string,offset:integer})},
  {name:'discussion',description:'Read loaded PR comments, submitted reviews, review threads and commits, paginated. Data reflects the last LazyReview refresh.',inputSchema:schema({offset:integer})},
  {name:'artifact_list',description:'List saved diagrams, pages and notes for this PR, including their revision and native session.',inputSchema:schema({})},
  {name:'artifact_read',description:'Read the source of a saved PR artifact before discussing or revising it.',inputSchema:{...schema({id:string}),required:['id']}},
  {name:'artifact_create',description:'Save a requested self-contained static HTML page, SVG diagram or Markdown note to this PR. Immutable: revisions create a new artifact. No scripts, external assets or network in previews. Never include secrets. Returns a shelf entry; the human opens it with g o. Available only in a user-requested turn.',inputSchema:{...schema({kind:{enum:['html','svg','markdown']},title:{type:'string',maxLength:100},summary:{type:'string',maxLength:600},content:{type:'string',maxLength:250000}}),required:['kind','title','summary','content']}},
].map(t=>({...t,annotations:{readOnlyHint:t.name!=='artifact_create',destructiveHint:false,openWorldHint:false}}));
let cachedToken;
function token() {
  if(cachedToken!==undefined)return cachedToken;
  cachedToken=process.env.GH_TOKEN||process.env.GITHUB_TOKEN||'';
  if(!cachedToken)try{cachedToken=execFileSync('gh',['auth','token','--hostname','github.com'],{encoding:'utf8',stdio:['ignore','pipe','ignore'],timeout:5000}).trim();}catch{}
  return cachedToken;
}
function page(items, offset=0, size=100) {
  if(!Number.isInteger(offset)||offset<0)throw Error('Invalid offset');
  return {items:items.slice(offset,offset+size),total:items.length,next:offset+size<items.length?offset+size:null};
}
// Receipts contain provenance only, never source or provider output. They are recorded
// after a successful tool call, so opening a tool or reading PR metadata cannot imply code coverage.
export function sourceReceipts(pr,name,args,value) {
  if(name==='changes'&&args.path&&value.patchAvailable&&value.patch)return [{kind:'patch',path:args.path,base:pr.base,commit:pr.head}];
  if(name==='read_file'&&value.text)return [{kind:'source',path:value.path,side:value.side,commit:value.commit,start:value.start,end:value.end}];
  if(name==='search_changes')return [...new Set(value.items.map(i=>i.path))].map(path=>({kind:'search',path,base:pr.base,commit:pr.head}));
  return [];
}
export async function recordReadReceipt(dir,pr,name,args,value) {
  if(!/^[a-f0-9-]{36}$/.test(pr.turnId||''))return;
  const additions=sourceReceipts(pr,name,args,value);if(!additions.length)return;
  const path=join(dir,'evidence.json');
  let previous;try{previous=JSON.parse(await readFile(path,'utf8'));}catch{}
  const sources=previous?.turnId===pr.turnId&&Array.isArray(previous.sources)?previous.sources:[];
  const seen=new Set(sources.map(s=>JSON.stringify(s)));
  for(const source of additions)if(!seen.has(JSON.stringify(source))){sources.push(source);seen.add(JSON.stringify(source));}
  const temp=path+'.'+randomUUID()+'.tmp';
  await writeFile(temp,JSON.stringify({turnId:pr.turnId,sources:sources.slice(0,256)}),{mode:0o600});
  try{await rename(temp,path);}finally{await unlink(temp).catch(()=>{});}
}
export async function callTool(pr, name, args={}, fetcher=fetch, auth=token, artifactDir) {
  const definition=tools.find(t=>t.name===name);
  if(!definition)throw Error('Unknown review tool');
  if(!args||typeof args!=='object'||Array.isArray(args)||Object.keys(args).some(k=>!(k in definition.inputSchema.properties)))throw Error('Invalid tool arguments');
  if(name.startsWith('artifact_')) {
    if(!artifactDir)throw Error('Artifact storage unavailable');
    if(name==='artifact_list')return {items:await listArtifacts(artifactDir)};
    if(name==='artifact_read')return readArtifact(artifactDir,args.id);
    if(pr.allowArtifacts!==true){const e=Error('Artifact creation is disabled during background preparation');e.code='ARTIFACT_INPUT';throw e;}
    return createArtifact(artifactDir,pr,args);
  }
  if(name==='review_context') {
    if(args.question!==undefined&&(typeof args.question!=='string'||args.question.length>10000))throw Error('Invalid question');
    let turn,active;
    if(artifactDir)try{active=JSON.parse(await readFile(join(artifactDir,'native-active.json'),'utf8'));turn=JSON.parse(await readFile(join(artifactDir,'native-turn.json'),'utf8'));}catch{}
    const matches=turn&&active&&turn.session===pr.chatSession&&turn.launch===active.launch&&!turn.cancelled;
    const items=currentAttachments(pr,matches?turn.items:pr.pendingContext||[]);
    const packet=patchEvidence(pr,args.question||turn?.prompt||'',pr.focus,items);
    const context=briefing(pr,null,packet)+'\n'+questionContext(pr,pr.focus,items);
    if(matches){
      const file=join(artifactDir,'native-turn.json'),temp=file+'.'+randomUUID()+'.tmp';
      await writeFile(temp,JSON.stringify({...turn,items,delivered:true}),{mode:0o600});await rename(temp,file);
    }
    return {context,notesDelivered:items.length,omittedPatchCount:packet.omittedPatchCount,excerptedPatchPaths:packet.excerptedPatchPaths};
  }
  const side=args.side||'RIGHT';
  if(!['LEFT','RIGHT'].includes(side))throw Error('Invalid side');
  const commit=side==='LEFT'?pr.base:pr.head;
  const repository=side==='LEFT'?pr.baseRepo:pr.headRepo;
  if(name==='overview')return {...Object.fromEntries(['url','title','body','author','base','head','baseRepo','headRepo','warnings','focus'].map(k=>[k,pr[k]])),fileCount:pr.files.length};
  if(name==='changes') {
    if(args.path){const f=pr.files.find(f=>f.filename===args.path);if(!f)throw Error('Unknown changed file');return {...f,patchAvailable:!!f.patch};}
    return page(pr.files.map(({patch,...f})=>({...f,patchAvailable:!!patch})),args.offset);
  }
  if(name==='discussion')return page([...(pr.conversation||[]).map(data=>({type:'comment',data})),...(pr.reviews||[]).map(data=>({type:'review',data})),...(pr.threads||[]).map(data=>({type:'thread',data})),...(pr.commits||[]).map(data=>({type:'commit',data}))],args.offset,20);
  if(name==='search_changes') {
    if(typeof args.query!=='string'||!args.query||args.query.length>500)throw Error('Provide a short literal query');
    return page(pr.files.flatMap(f=>(f.patch||'').split('\n').filter(line=>line.includes(args.query)).map(line=>({path:f.filename,line}))),args.offset,40);
  }
  if(name==='read_file') {
    const start=args.start??1,end=args.end??start+199;
    if(!Number.isInteger(start)||!Number.isInteger(end)||start<1||end<start||end-start>=300)throw Error('Use a range of 1–300 lines');
    const {content}=await readSnapshotContent({repository,commit,path:args.path},auth(),fetcher);
    const lines=content.split('\n');
    return {path:args.path,side,commit,totalLines:lines.length,start,end:Math.min(end,lines.length),text:lines.slice(start-1,end).map((line,i)=>`${start+i}: ${line}`).join('\n')};
  }
  if(!/^[\w.-]+\/[\w.-]+$/.test(repository)||repository.split('/').some(s=>s==='.'||s==='..')||!/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/i.test(commit))throw Error('Invalid snapshot');
  if(args.query!==undefined&&(typeof args.query!=='string'||args.query.length>500))throw Error('Invalid path query');
  const credential=auth();
  const response=await fetcher(`https://api.github.com/repos/${repository}/git/trees/${commit}?recursive=1`,{method:'GET',redirect:'error',headers:{Accept:'application/vnd.github+json','X-GitHub-Api-Version':'2026-03-10',...(credential?{Authorization:`Bearer ${credential}`}:{})},signal:AbortSignal.timeout(10000)});
  if(!response.ok)throw Error('Repository tree unavailable');
  const data=await response.json();
  if(!Array.isArray(data.tree))throw Error('Invalid repository tree');
  return {...page(data.tree.filter(f=>f.type==='blob'&&f.path.includes(args.query||'')).map(f=>f.path),args.offset,200),commit,truncated:!!data.truncated};
}
if(process.argv[1] && import.meta.url===pathToFileURL(realpathSync(process.argv[1])).href) {
  const source=process.argv[2];
  for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})) {
    let req;
    try {
      if(line.length>2000000)throw Error('Request too large');
      req=JSON.parse(line);if(req.id===undefined)continue;
      let result;
      if(req.method==='initialize')result={protocolVersion:'2025-11-25',capabilities:{tools:{}},serverInfo:{name:'lazyreview',version:'0.3.0'}};
      else if(req.method==='ping')result={};
      else if(req.method==='tools/list')result={tools};
      else if(req.method==='tools/call') {
        try {
          const pr=JSON.parse(await readFile(source,'utf8'));
          const value=await callTool(pr,req.params.name,req.params.arguments,fetch,token,dirname(source));
          await recordReadReceipt(dirname(source),pr,req.params.name,req.params.arguments||{},value);
          result={content:[{type:'text',text:JSON.stringify(value)}]};
        }
        catch(error) {result={isError:true,content:[{type:'text',text:error.code==='ARTIFACT_INPUT'?error.message:'Tool unavailable. Check its arguments; content may be missing, oversized or inaccessible. Do not assume contents or claim an artifact was saved.'}]};}
      } else {process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:req.id,error:{code:-32601,message:'Method not found'}})+'\n');continue;}
      process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:req.id,result})+'\n');
    } catch {process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:req?.id??null,error:{code:-32600,message:'Invalid request'}})+'\n');}
  }
}
