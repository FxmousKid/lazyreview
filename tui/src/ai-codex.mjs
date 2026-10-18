import {spawn} from 'node:child_process';

// Use Codex's public protocol to reserve a resumable, named session before its
// native TUI opens. No inference request, hook trust, or transcript scraping.
// Verified with codex-cli 0.163.0. See https://developers.openai.com/codex/app-server.
export async function createCodexSession({command='codex',cwd,title,model,timeoutMs=15000}) {
  if(typeof command!=='string'||!command||typeof cwd!=='string'||!cwd||typeof title!=='string'||!title.trim())throw new Error('Codex session setup requires a command, working directory, and title.');
  if(!Number.isFinite(timeoutMs)||timeoutMs<1||timeoutMs>120000)throw new Error('Codex session setup timeout must be 1–120000 ms.');
  const child=spawn(command,['app-server','--stdio','-c','features.hooks=false'],{
    cwd,env:process.env,stdio:['pipe','pipe','ignore'],windowsHide:true,
  });
  let nextId=0,buffer='',failure=null,closing=false;
  const pending=new Map();
  let resolveClosed;
  const closed=new Promise(resolve=>{resolveClosed=resolve;});
  const fail=error=>{failure=error;buffer='';for(const {reject} of pending.values())reject(error);pending.clear();};
  const send=message=>{if(failure)throw failure;child.stdin.write(JSON.stringify(message)+'\n');};
  const request=(method,params)=>new Promise((resolve,reject)=>{
    if(failure)return reject(failure);
    const id=++nextId;pending.set(id,{resolve,reject,method});
    try{send({id,method,params});}catch(error){pending.delete(id);reject(error);}
  });
  child.on('error',()=>fail(new Error('Could not start Codex app-server. Check the configured executable.')));
  child.stdin.on('error',()=>{if(!closing)fail(new Error('Codex session setup connection closed.'));});
  child.on('close',()=>{resolveClosed();if(!closing)fail(new Error('Codex app-server exited before session setup completed.'));});
  child.stdout.setEncoding('utf8');
  child.stdout.on('data',chunk=>{
    if(failure)return;
    buffer+=chunk;
    if(buffer.length>2_000_000){fail(new Error('Codex session setup exceeded its response limit.'));return;}
    let newline;
    while((newline=buffer.indexOf('\n'))>=0){
      const line=buffer.slice(0,newline);buffer=buffer.slice(newline+1);
      let message;try{message=JSON.parse(line);}catch{continue;}
      if(message&&message.method&&message.id!==undefined){
        // Bootstrap must never authorize an unexpected provider tool/approval.
        try{send({id:message.id,error:{code:-32601,message:'Not available during session setup'}});}catch{}
        continue;
      }
      const waiting=pending.get(message?.id);if(!waiting)continue;
      pending.delete(message.id);
      if(message.error)waiting.reject(new Error(`Codex ${waiting.method} failed. Check the CLI version and local configuration.`));
      else if(!Object.hasOwn(message,'result'))waiting.reject(new Error('Codex session setup returned an invalid response.'));
      else waiting.resolve(message.result);
    }
  });
  const timer=setTimeout(()=>fail(new Error('Codex session setup timed out. Retry or check the CLI configuration.')),timeoutMs);
  try{
    await request('initialize',{clientInfo:{name:'lazyreview',title:'LazyReview',version:'0.1.0'}});
    send({method:'initialized',params:{}});
    // Empty TOML tables merge with inherited settings; they do not clear them.
    // Read effective public config and disable each inherited server/plugin by
    // name for this bootstrap process only. Never log this potentially secret data.
    const effective=await request('config/read',{cwd,includeLayers:false});
    if(!effective?.config||typeof effective.config!=='object'||Array.isArray(effective.config))throw new Error('Codex did not return its effective setup configuration.');
    const disabled=key=>Object.fromEntries(Object.keys(effective?.config?.[key]||{}).map(name=>[name,{enabled:false}]));
    const params={cwd,ephemeral:false,sandbox:'read-only',approvalPolicy:'on-request',config:{
      mcp_servers:disabled('mcp_servers'),plugins:disabled('plugins'),features:{hooks:false},
    }};
    if(model&&model!=='default')params.model=model;
    const result=await request('thread/start',params);
    const id=result?.thread?.id;
    if(typeof id!=='string'||!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id))throw new Error('Codex did not return a resumable session ID.');
    await request('thread/name/set',{threadId:id,name:title.replace(/[\x00-\x1f\x7f]/g,'').slice(0,180)});
    return id;
  }finally{
    clearTimeout(timer);closing=true;child.stdin.end();
    // EOF is the clean shutdown. Escalate only this owned process if it stalls.
    const terminate=setTimeout(()=>child.kill('SIGTERM'),300);
    const kill=setTimeout(()=>child.kill('SIGKILL'),1000);
    await closed;clearTimeout(terminate);clearTimeout(kill);
  }
}
