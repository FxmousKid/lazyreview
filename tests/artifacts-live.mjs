// Opt-in: two bounded Claude turns against a public PR. No GitHub writes.
import {execFileSync} from 'node:child_process';
import {mkdtemp,rm,readFile,writeFile,mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {executeGithub} from '../lib/github.ts';
import {Companion} from '../tui/src/ai-session.mjs';
import {listArtifacts,readArtifact,viewer} from '../tui/src/artifacts.mjs';
if(!process.argv.includes('--live'))throw Error('Use --live for two bounded real Claude turns');
const token=process.env.GH_TOKEN||process.env.GITHUB_TOKEN||execFileSync('gh',['auth','token','--hostname','github.com'],{encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim();
const pr=await executeGithub({operation:'load',url:'https://github.com/primer/react/pull/8520'},token);
const root=await mkdtemp(join(tmpdir(),'rw-artifact-live-')),events=[];
const config={enabled:true,profile:'artifact-validation',command:'claude',model:'sonnet',autoPrepare:false,maxBudgetUsd:1};
let c=new Companion(e=>events.push({...e,at:performance.now()}),{root});
try {
 await c.open({pr,config,binding:'1'});const start=performance.now(),session=c.current.record.session;
 await c.send({binding:'1',prompt:'Help me understand this PR overall. Read overview and changes. Then use artifact_create to save TWO small outputs: an SVG diagram explaining the main behavior or review flow, and a self-contained HTML page explaining the purpose, main tradeoff, and two review questions. Cite the actual filenames and distinguish any inference. Use readable labels and a dark GitHub-like palette. Keep each output under 6000 characters, no JavaScript or external dependencies. Finally answer in two sentences naming what you saved.',focus:{scope:'pr'}});
 await c.active?.done;
 let error=events.find(e=>e.error);if(error)throw Error(error.error);
 let items=await listArtifacts(c.current.dir);
 if(!['svg','html'].every(kind=>items.some(i=>i.kind===kind)))throw Error('Claude did not create both outputs');
 const directory=c.current.dir;
 const names=items.map(i=>i.title);
 const calls=events.filter(e=>e.type==='activity').map(e=>e.tool);
 console.log(JSON.stringify({firstTurnTools:calls,artifactsSaved:items.length}));
 const firstDelta=events.find(e=>e.type==='delta');
 await c.close();c=new Companion(e=>events.push({...e,at:performance.now()}),{root});
 await c.open({pr,config,binding:'2'});if(c.current.record.session!==session||c.current.artifacts.length!==2)throw Error('Session/shelf did not resume');
 await c.send({binding:'2',prompt:'Revisit our saved work. Use artifact_list and artifact_read to inspect the diagram you made. In one sentence tell me which part of that diagram is most useful for a reviewer unfamiliar with the repository. Do not create another output.',focus:{scope:'pr'}});
 await c.active?.done;error=events.find(e=>e.error);if(error)throw Error(error.error);
 if(!events.some(e=>e.tool?.endsWith('__artifact_read')))throw Error('Did not read back saved artifact');
 const outFlag=process.argv.indexOf('--out');
 if(outFlag>=0){const out=process.argv[outFlag+1];await mkdir(out,{recursive:true});for(const item of items){const source=await readArtifact(directory,item.id);await writeFile(join(out,item.kind==='svg'?'diagram.html':'explainer.html'),viewer(source));}}
 console.log(JSON.stringify({artifacts:items.map(v=>({kind:v.kind,title:v.title,bytes:v.bytes})),nativeResume:true,artifactReadBack:true,toolCalls:calls,firstDeltaMs:Math.round(firstDelta.at-start),turns:2,githubWrites:0}));
} finally {await c.close();await rm(root,{recursive:true,force:true});}
