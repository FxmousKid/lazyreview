// PR-local outputs only. No model-supplied filesystem paths or arbitrary command execution.
import {mkdir,open,lstat,readdir,writeFile} from 'node:fs/promises';
import {constants} from 'node:fs';
import {join} from 'node:path';
import {randomUUID,createHash} from 'node:crypto';
import {spawn} from 'node:child_process';
const ID=/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/;
const LIMIT=250_000;
const clean=s=>s.replace(/[\x00-\x08\x0b-\x1f\x7f]/g,'');
const esc=s=>String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;');
const hash=s=>createHash('sha256').update(s).digest('hex');
function fail(message){const e=Error(message);e.code='ARTIFACT_INPUT';throw e;}
async function directory(path,create=false){
  if(create)await mkdir(path,{recursive:true,mode:0o700});
  const info=await lstat(path);if(!info.isDirectory()||info.isSymbolicLink())fail('Artifact directory is unavailable');return path;
}
async function root(dir,create=false){return directory(join(dir,'artifacts'),create);}
async function entry(dir,id){if(!ID.test(id))fail('Invalid artifact ID');return directory(join(await root(dir),id));}
async function read(path,max=LIMIT*6+1000){
  const file=await open(path,constants.O_RDONLY|constants.O_NOFOLLOW);
  try{const stat=await file.stat();if(!stat.isFile()||stat.size>max)fail('Artifact is too large');return JSON.parse(await file.readFile('utf8'));}finally{await file.close();}
}
export async function listArtifacts(dir){
  let base;try{base=await root(dir);}catch(e){if(e.code==='ENOENT')return [];throw e;}
  const entries=await readdir(base,{withFileTypes:true});const out=[];
  for(const file of entries.slice(0,300)){
    if(!file.isDirectory()||!ID.test(file.name))continue;
    try{const item=await read(join(base,file.name,'meta.json'),5000);if(item.id===file.name&&['html','svg','markdown'].includes(item.kind)&&typeof item.title==='string'&&typeof item.summary==='string'&&typeof item.created==='string'&&typeof item.head==='string'&&typeof item.url==='string')out.push(item);}catch{}
  }
  return out.sort((a,b)=>b.created.localeCompare(a.created));
}
export async function readArtifact(dir,id){
  const folder=await entry(dir,id);const meta=await read(join(folder,'meta.json'),5000),source=await read(join(folder,'source.json'));
  if(meta.id!==id||!['html','svg','markdown'].includes(meta.kind)||!['title','summary','head','url','created'].every(k=>typeof meta[k]==='string')||typeof source.content!=='string'||Buffer.byteLength(source.content)>LIMIT||hash(source.content)!==meta.sha256)fail('Artifact contents do not match');
  return {...meta,content:source.content};
}
export function viewer(artifact){
  // The generated page is inert: scripts, forms, popups, top navigation, remote assets
  // and network calls are disabled. The wrapper remains a portable, standalone HTML file.
  const policy="default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; media-src 'none'; base-uri 'none'; form-action 'none'";
  const content=artifact.kind==='markdown'?`<pre style="white-space:pre-wrap;font:16px/1.65 ui-monospace,monospace">${esc(artifact.content)}</pre>`:artifact.content;
  const document=`<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${esc(policy)}"><meta name="referrer" content="no-referrer"><style>:root{color-scheme:dark}body{margin:0;padding:24px;background:#0d1117;color:#e6edf3;font:16px/1.6 system-ui,sans-serif}svg{display:block;max-width:100%;height:auto}a{color:#79c0ff}pre{overflow:auto}table{border-collapse:collapse}th,td{padding:8px;border:1px solid #30363d}</style></head><body>${content}</body></html>`;
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="referrer" content="no-referrer"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; frame-src 'self' about:; script-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'"><title>${esc(artifact.title)}</title><style>:root{color-scheme:dark}*{box-sizing:border-box}body{margin:0;background:#080b10;color:#e6edf3;font:15px/1.5 system-ui,sans-serif}header{max-width:1200px;margin:auto;padding:22px 28px 16px}small,summary{color:#9198a1}h1{font-size:25px;margin:4px 0;font-weight:600}p{margin:7px 0;color:#b1bac4}iframe{display:block;width:calc(100% - 40px);height:75vh;margin:0 20px;border:1px solid #30363d;border-radius:9px;background:#0d1117}details{margin:18px 28px}pre{white-space:pre-wrap;overflow-wrap:anywhere}a{color:#79c0ff} @media print{iframe{height:90vh;border:0}details{display:none}}</style></head><body><header><small>REVIEW LAZYREVIEW · ${esc(artifact.kind.toUpperCase())} · ${esc(artifact.head.slice(0,7))}</small><h1>${esc(artifact.title)}</h1><p>${esc(artifact.summary)}</p><small>${esc(artifact.url)} · ${esc(artifact.created.slice(0,10))}</small></header><iframe title="${esc(artifact.title)}" sandbox="" referrerpolicy="no-referrer" srcdoc="${esc(document)}"></iframe><details><summary>Source</summary><pre>${esc(artifact.content)}</pre></details></body></html>`;
}
export async function createArtifact(dir,pr,args){
  const {kind,title,summary,content}=args;
  if(!['html','svg','markdown'].includes(kind))fail('Use html, svg or markdown');
  if(typeof title!=='string'||!title.trim()||title.length>100||typeof summary!=='string'||!summary.trim()||summary.length>600)fail('Provide a title (1–100 characters) and summary (1–600 characters)');
  if(typeof content!=='string'||!content.trim()||Buffer.byteLength(content)>LIMIT)fail('Provide up to 250 KB of content');
  if(kind==='svg'&&!/^\s*<svg\b[\s\S]*<\/svg>\s*$/.test(content))fail('Provide a complete SVG element with a viewBox');
  if(!/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/i.test(pr.head||'')||typeof pr.url!=='string')fail('Artifact needs a PR revision');
  const base=await root(dir,true);
  if((await readdir(base)).length>=200)fail('This PR already has 200 artifacts');
  const id=randomUUID(),folder=join(base,id);await mkdir(folder,{mode:0o700});
  const value={id,kind,title:clean(title.trim()),summary:clean(summary.trim()),url:pr.url,base:pr.base,head:pr.head,session:pr.chatSession||'',created:new Date().toISOString(),bytes:Buffer.byteLength(content),sha256:hash(content)};
  await writeFile(join(folder,'source.json'),JSON.stringify({content}),{mode:0o600,flag:'wx'});
  await writeFile(join(folder,'preview.html'),viewer({...value,content}),{mode:0o600,flag:'wx'});
  // Metadata is the commit marker; unfinished outputs never appear on the shelf.
  await writeFile(join(folder,'meta.json'),JSON.stringify(value),{mode:0o600,flag:'wx'});
  return value;
}
export async function openArtifact(dir,id,launch=launchFile){
  const artifact=await readArtifact(dir,id);const folder=await entry(dir,id);
  const path=join(folder,'preview.html');
  const file=await open(path,constants.O_WRONLY|constants.O_CREAT|constants.O_TRUNC|constants.O_NOFOLLOW,0o600);
  try{await file.writeFile(viewer(artifact));}finally{await file.close();}
  await launch(path);return artifact;
}
function launchFile(path){
  const command=process.platform==='darwin'?'open':process.platform==='win32'?'explorer.exe':'xdg-open';
  return new Promise((resolve,reject)=>{
    const child=spawn(command,[path],{stdio:'ignore',shell:false});
    child.once('error',()=>reject(Error('Could not open the artifact preview')));
    child.once('close',code=>code===0?resolve():reject(Error('Could not open the artifact preview')));
  });
}
