#!/usr/bin/env node
// Deterministic native-session simulator. Never contacts a provider.
import {readFile,writeFile} from 'node:fs/promises';
import {spawn} from 'node:child_process';
const args=process.argv.slice(2),value=flag=>args[args.indexOf(flag)+1];
let prompt='';for await(const chunk of process.stdin)prompt+=chunk;
const id=value(args.includes('--resume')?'--resume':'--session-id');
const emit=v=>process.stdout.write(JSON.stringify({...v,session_id:id})+'\n');
let saved={requests:[]};try{saved=JSON.parse(await readFile('fixture.json','utf8'));}catch{}
saved.requests.push({id,resume:args.includes('--resume'),prompt,args});await writeFile('fixture.json',JSON.stringify(saved));
if(prompt.includes('SIMULATE_ERROR')){emit({type:'result',is_error:true,result:'failed'});process.exit(1);}
emit({type:'system',subtype:'init'});
if(prompt.includes('SIMULATE_ARTIFACT')){
 const config=JSON.parse(await readFile(value('--mcp-config'),'utf8'));
 const server=config.mcpServers.review;
 const child=spawn(server.command,server.args,{stdio:['pipe','pipe','pipe']});
 let output='';child.stdout.on('data',v=>output+=v);child.stderr.resume();
 const done=new Promise((resolve,reject)=>{child.once('close',resolve);child.once('error',reject);});
 emit({type:'stream_event',event:{type:'content_block_start',content_block:{type:'tool_use',name:'mcp__review__artifact_create'}}});
 child.stdin.end(JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'artifact_create',arguments:{kind:'svg',title:'Fixture diagram',summary:'An isolated end-to-end output.',content:'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 80"><text x="10" y="35">PR → evidence</text></svg>'}}})+'\n');
 if(await done!==0||JSON.parse(output.trim()).result.isError)throw Error('Fixture artifact failed');
}
if(prompt.includes('SIMULATE_CHILD')){
 const child=spawn(process.execPath,['-e',"process.on('SIGINT',()=>{});setInterval(()=>{},1000)"],{stdio:'ignore'});
 await writeFile('fixture-child.json',JSON.stringify({pid:child.pid}));
 await new Promise(resolve=>setTimeout(resolve,10000));
}
if(prompt.includes('SIMULATE_SLOW')||(!args.includes('--resume')&&prompt.includes('Prepare to accompany')))await new Promise(resolve=>setTimeout(resolve,150));
emit({type:'stream_event',event:{type:'content_block_delta',delta:{type:'text_delta',text:'A streamed '}}});
await new Promise(resolve=>setTimeout(resolve,40));
emit({type:'stream_event',event:{type:'content_block_delta',delta:{type:'text_delta',text:'answer.'}}});
emit({type:'result',is_error:false,result:'A streamed answer.'});
