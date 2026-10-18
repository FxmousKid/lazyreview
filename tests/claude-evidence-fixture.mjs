// Deterministic native CLI fixture that exercises the real evidence MCP transport.
import {spawn} from 'node:child_process';
import {readFile,writeFile} from 'node:fs/promises';
const args=process.argv.slice(2),get=flag=>args[args.indexOf(flag)+1];
let prompt='';for await(const chunk of process.stdin)prompt+=chunk;
await writeFile('evidence-request.txt',prompt);
const session_id=get(args.includes('--resume')?'--resume':'--session-id');
const emit=value=>process.stdout.write(JSON.stringify({session_id,...value})+'\n');
emit({type:'system',subtype:'init'});
if(prompt.includes('READ_PATCH')) {
  const {mcpServers:{review:server}}=JSON.parse(await readFile(get('--mcp-config'),'utf8'));
  const child=spawn(server.command,server.args,{stdio:['pipe','pipe','ignore']});
  let out='';child.stdout.on('data',chunk=>out+=chunk);
  const done=new Promise((resolve,reject)=>{child.once('close',resolve);child.once('error',reject);});
  emit({type:'stream_event',event:{type:'content_block_start',content_block:{type:'tool_use',name:'mcp__review__changes'}}});
  child.stdin.end(JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'changes',arguments:{path:'db/migrations/member.sql'}}})+'\n');
  if(await done!==0||JSON.parse(out.trim()).result.isError)throw Error('MCP evidence fixture failed');
}
if(prompt.includes('FAIL_TURN')){emit({type:'result',is_error:true,result:'failed'});process.exit(1);}
if(prompt.includes('STOP_TURN'))await new Promise(resolve=>setTimeout(resolve,10000));
emit({type:'stream_event',event:{type:'content_block_delta',delta:{type:'text_delta',text:'Access requires a member.'}}});
emit({type:'result',is_error:false,result:'Access requires a member.'});
