import {realpathSync,existsSync} from 'node:fs';
// Native Claude sessions own model history. session.json is only linkage + UI display cache.
import {mkdir,open,readFile,writeFile,rename,unlink,chmod} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import {randomUUID,createHash} from 'node:crypto';
import {createInterface} from 'node:readline';
import {dirname,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {homedir} from 'node:os';
import {SYSTEM,identity,snapshot,revision,briefing,questionContext,patchEvidence,currentAttachments} from './ai-context.mjs';
import {listArtifacts,openArtifact} from './artifacts.mjs';
import {terminalLaunch,nativeEvents} from './ai-terminal.mjs';
import {provider} from './ai-providers.mjs';
const here=dirname(fileURLToPath(import.meta.url));
export async function atomicJson(path,value) {
  const temp=path+'.'+randomUUID()+'.tmp';
  await writeFile(temp,JSON.stringify(value),{mode:0o600});
  try {await rename(temp,path);} finally {await unlink(temp).catch(()=>{});}
}
export async function acquireLock(dir) {
  const path=join(dir,'lock');
  for(let attempt=0;attempt<2;attempt++) {
    try {const file=await open(path,'wx',0o600);await file.writeFile(String(process.pid));await file.close();return async()=>{await unlink(path).catch(()=>{});};}
    catch(error) {
      if(error.code!=='EEXIST')throw error;
      const pid=Number(await readFile(path,'utf8').catch(()=>''));
      if(!Number.isInteger(pid)||pid<=0)throw Error('This chat has an unreadable lock. Close the other LazyReview before retrying.');
      try{process.kill(pid,0);throw Error('This chat is already open in another LazyReview.');}catch(e){if(e.code!=='ESRCH')throw e;}
      await unlink(path).catch(()=>{});
    }
  }
  throw Error('Could not lock the PR chat');
}
export function cliArgs(config,record,mcp,prepare) {
  return ['-p','--output-format','stream-json','--verbose','--include-partial-messages',
    record.started?'--resume':'--session-id',record.session,
    '--model',config.model&&config.model!=='default'?config.model:'sonnet','--effort','low','--tools','',
    '--allowedTools','mcp__review__*','--strict-mcp-config','--mcp-config',mcp,
    '--permission-mode','dontAsk','--permission-prompts','none',
    '--setting-sources','','--settings',JSON.stringify({disableAllHooks:true,enabledPlugins:{}}),
    '--disable-slash-commands','--no-chrome','--system-prompt',SYSTEM,
    '--max-turns',prepare?'3':'12','--max-budget-usd',String(config.maxBudgetUsd??1)];
}
export class Companion {
  constructor(emit,{root,legacyRoot,spawnCli=spawn}={}) {
    const state=process.env.XDG_STATE_HOME||join(homedir(),'.local/state');
    this.emit=emit;this.root=root||join(state,'lazyreview','claude');this.spawnCli=spawnCli;
    this.legacyRoot=legacyRoot??(root?null:join(state,'review-workbench','claude'));
    this.current=null;this.active=null;
  }
  event(type,data={}) {if(this.current)this.emit({binding:this.current.binding,type,...data});}
  async save() {
    const c=this.current;if(!c)return;
    while(c.record.messages.length>200 || (JSON.stringify(c.record.messages).length>240000 && c.record.messages.length>2)) {c.record.messages.shift();c.record.displayTruncated=true;}
    await atomicJson(join(c.dir,'session.json'),c.record);
  }
  state(status,error='') {
    const r=this.current.record;
    this.event('state',{status,error,session:r.session,messages:r.messages,summary:r.summary||'',displayTruncated:!!r.displayTruncated,artifacts:this.current.artifacts||[],evidence:this.current.evidence||null,prepared:r.revision===revision(this.current.pr)});
  }
  async open(input) {
    await this.close();
    const config=input.config;
    if(!config?.enabled) return;
    if(typeof config.command!=='string'||!config.command||typeof config.model!=='string'||!config.model||!Number.isFinite(config.maxBudgetUsd)||config.maxBudgetUsd<=0||config.maxBudgetUsd>100)throw Error('Invalid AI configuration');
    const adapter=provider(config);
    if(!adapter.paper && config.interface!=="terminal")throw Error(`${adapter.label} requires terminal mode`);
    const pr=snapshot(input.pr,input.focus);
    const key=identity(pr,config);
    // Keep existing native cwd, session IDs and locks intact across the rename.
    const canonical=join(this.root,key),legacy=this.legacyRoot&&join(this.legacyRoot,key);
    const dir=!existsSync(canonical)&&legacy&&existsSync(legacy)?legacy:canonical;
    await mkdir(dir,{recursive:true,mode:0o700});await chmod(dir,0o700);
    const release=await acquireLock(dir);
    this.current={binding:input.binding,dir,release,config,pr,record:null,artifacts:[]};
    try {
      let record;
      try {record=JSON.parse(await readFile(join(dir,'session.json'),'utf8'));}
      catch(error) {if(error.code!=='ENOENT')throw Error('Saved chat metadata is unreadable; restore it before continuing.');}
      if(record && (!/^[a-f0-9-]{36}$/i.test(record.session)||!Array.isArray(record.messages)||record.key!==key))throw Error('Saved chat metadata does not match this PR/profile');
      this.current.record=record||{version:1,key,session:randomUUID(),started:false,revision:null,messages:[],summary:''};
      this.current.evidence=record?.evidence?.revision===revision(pr)?record.evidence:null;
      await this.save();
      this.current.artifacts=await listArtifacts(dir);
      await atomicJson(join(dir,'snapshot.json'),{...pr,allowArtifacts:false});
      await atomicJson(join(dir,'mcp.json'),{mcpServers:{review:{command:process.execPath,args:[join(here,'ai-tools.mjs'),join(dir,'snapshot.json')]}}});
      this.state(record?'Ready':'Connected');
      if(config.interface!=='terminal' && config.autoPrepare && this.current.record.revision!==revision(pr))this.run('',true);
    } catch(error) {await release();this.current=null;throw error;}
  }
  async send(input) {
    if(!this.current||input.binding!==this.current.binding)return;
    if(this.active && !this.active.prepare)throw Error('Claude is answering. Stop it before sending another question.');
    const prompt=String(input.prompt||'').trim();
    if(!prompt||prompt.length>10000)throw Error('Ask a question of 1–10,000 characters');
    await this.stop();
    this.current.pr.focus=input.focus||{};
    this.run(prompt,false,input.context||[]);
  }
  async fresh(binding) {
    if(!this.current||binding!==this.current.binding)return;
    await this.stop();await this.pollNative();
    const c=this.current;
    await atomicJson(join(c.dir,`archived-${c.record.session}.json`),c.record);
    c.record={version:1,key:c.record.key,session:randomUUID(),started:false,revision:null,messages:[],summary:''};
    c.evidence=null;c.nativeLaunch=null;
    await this.save();this.state('Connected');
    if(c.config.interface!=='terminal'&&c.config.autoPrepare)this.run('',true);
  }
  async nativeSnapshot(){
    const c=this.current;if(!c)return;
    let chatSession=c.record.session;
    // A native /clear can submit its first prompt before the receipt poll runs.
    try{const active=JSON.parse(await readFile(join(c.dir,'native-active.json'),'utf8'));if(c.nativeLaunch&&active.launch===c.nativeLaunch)chatSession=active.session;}catch{}
    await atomicJson(join(c.dir,'snapshot.json'),{...c.pr,allowArtifacts:true,chatSession,pendingContext:c.nativeContext||[]});
  }
  async terminal(binding){
    if(!this.current||binding!==this.current.binding)return;
    await this.stop();await this.pollNative();await this.nativeSnapshot();
    const launch=await terminalLaunch(this.current,here);
    await this.save();await this.nativeSnapshot();
    this.state('Ready');this.event('terminal_launch',launch);
    clearInterval(this.nativePoll);
    this.nativePoll=setInterval(()=>{this.pollNative().catch(()=>{});},250);
  }
  async nativeContext(input){
    if(!this.current||input.binding!==this.current.binding)return;
    this.current.nativeContext=currentAttachments(this.current.pr,input.context||[]);
    this.current.pr.focus=input.focus||{};
    await this.nativeSnapshot();
  }
  async pollNative(){
    // close/fresh/relaunch must drain the same read, not skip it. Otherwise an
    // old Stop can finish after open() and bind its consumption event to a new PR.
    if(this.nativePollTask)return this.nativePollTask;
    const c=this.current;if(!c)return;
    const task=this.readNative(c);this.nativePollTask=task;
    try{await task;}finally{if(this.nativePollTask===task)this.nativePollTask=null;}
  }
  async readNative(c){
    for(const e of await nativeEvents(c.dir)){
      if(this.current!==c)return;
      if(e.launch&&e.launch!==c.nativeLaunch)continue;
      if(e.kind==='SessionStart'&&e.previousSession===c.record.session&&/^[a-f0-9-]{36}$/i.test(e.session)){
        await atomicJson(join(c.dir,`archived-${c.record.session}.json`),c.record);
        if(this.current!==c)return;
        c.record={version:1,key:c.record.key,session:e.session,started:true,revision:null,messages:[],summary:''};
        c.evidence=null;await this.save();
        if(this.current!==c)return;
        await this.nativeSnapshot();
        if(this.current!==c)return;
        this.state('Ready');
      }
      if(e.session!==c.record.session)continue;
      if(['SessionStart','UserPromptSubmit','Stop'].includes(e.kind))c.record.started=true;
      if(e.kind==='Stop'&&e.turn&&e.turn.id!==c.record.nativeLastTurn){
        c.record.nativeLastTurn=e.turn.id;c.record.revision=e.turn.revision;
        c.record.messages.push({role:'user',content:e.turn.prompt},{role:'assistant',content:e.answer});
        await this.save();
        if(this.current!==c)return;
        this.event('native_complete',{items:e.turn.items});
        await this.refreshArtifacts(c);
        if(this.current!==c)return;
      }
      this.event('native_status',{kind:e.kind});await this.save();
    }
  }
  async openOutput(binding,id) {
    if(!this.current||binding!==this.current.binding)return;
    try{const item=await openArtifact(this.current.dir,id);this.event('artifact_opened',{title:item.title});}
    catch{this.event('artifact_error',{error:'Could not open this artifact. Check that its source and a browser are available.'});}
  }
  async refreshArtifacts(c) {
    try {const items=await listArtifacts(c.dir);if(JSON.stringify(items)!==JSON.stringify(c.artifacts)){c.artifacts=items;if(this.current===c)this.event('artifacts',{items});}}catch{}
  }
  async refreshEvidence(c,active) {
    try {
      const receipt=JSON.parse(await readFile(join(c.dir,'evidence.json'),'utf8'));
      if(receipt.turnId!==active.turnId||!Array.isArray(receipt.sources)||this.current!==c)return;
      const evidence={...c.evidence,readSources:receipt.sources,readSourceCount:receipt.sources.filter(s=>s.kind==='source').length,readPatchCount:receipt.sources.filter(s=>s.kind==='patch').length};
      if(JSON.stringify(evidence)!==JSON.stringify(c.evidence)){c.evidence=evidence;this.event('evidence',{evidence});}
    }catch{}
  }
  run(prompt,prepare,attachments=[]) {
    const c=this.current,r=c.record;
    const accepted=currentAttachments(c.pr,attachments),turnId=randomUUID();
    // IDs contain SHA/path/range, so a valid deep path can exceed ordinary ID lengths.
    // The accepted attachment packet is already bounded as a whole.
    const contextIds=[...new Set(accepted.map(a=>a.id).filter(id=>typeof id==='string'&&id.length>0))];
    const packet=patchEvidence(c.pr,prompt,c.pr.focus,accepted);
    const prior=r.codeContext?.revision===revision(c.pr)?r.codeContext:{providedPatchPaths:[],readSources:[]};
    const known=new Set(prior.providedPatchPaths);
    const fingerprint=p=>createHash('sha256').update(p.path+'\0'+p.patch).digest('hex');
    const fingerprints=new Set(prior.patchFingerprints||[]);
    const fresh={...packet,patches:packet.patches.filter(p=>!fingerprints.has(fingerprint(p)))};
    const sources=prior.readSources||[];
    const context=r.revision===revision(c.pr)?`Current comparison: ${revision(c.pr)}.\nAdditional code evidence (JSON): ${JSON.stringify(fresh.patches)}`:briefing(c.pr,r.revision,fresh);
    const provenance=`Code context already supplied in this native session at this comparison: ${JSON.stringify([...known])}. Earlier successful code reads: ${JSON.stringify(sources)}. Reuse that evidence if it supports this question; no redundant tool calls are required. ${packet.omittedPatchCount} changed files are outside this question's bounded patch packet. Excerpted patches: ${JSON.stringify(packet.excerptedPatchPaths)}. Read omitted or excerpted code with changes/read_file before making claims beyond the supplied lines.`;
    const request=prepare?`${context}\n${provenance}\nPrepare to accompany this reviewer. Inspect the supplied implementation patches. Read at most two surrounding files when needed to explain them. Give a brief orientation (at most 120 words), including the concrete behavior visible in code. Do not perform a full review. Stop after that.`:
      `${context}\n${provenance}\n${questionContext(c.pr,c.pr.focus,accepted)}\nReviewer question:\n${prompt}`;
    if(!prepare)r.messages.push({role:'user',content:prompt,contextIds});
    c.evidence={revision:revision(c.pr),providedPatchPaths:fresh.patches.map(p=>p.path),reusedPatchPaths:[...known],omittedPatchCount:packet.omittedPatchCount,excerptedPatchPaths:packet.excerptedPatchPaths,readSources:[],readSourceCount:0,readPatchCount:0};
    this.event('evidence',{evidence:c.evidence});
    this.state(prepare?'Preparing':'Thinking');
    const active={prepare,turnId,contextIds,cancelled:false,answer:'',result:null,child:null,done:null};
    this.active=active;
    active.done=(async()=>{
      let timer, poll, exited;
      try {
        await this.save();
        await atomicJson(join(c.dir,'evidence.json'),{turnId,sources:[]});
        await atomicJson(join(c.dir,'snapshot.json'),{...c.pr,allowArtifacts:!prepare,chatSession:r.session,turnId});
        if(active.cancelled)return;
        poll=setInterval(()=>{void this.refreshArtifacts(c);void this.refreshEvidence(c,active);},500);
        const env={...process.env,CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC:'1'};
        // Nested invocation marker belongs to the parent process, not this independent session.
        delete env.CLAUDECODE;
        const child=this.spawnCli(c.config.command,cliArgs(c.config,r,join(c.dir,'mcp.json'),prepare),{cwd:c.dir,env,stdio:['pipe','pipe','pipe'],detached:process.platform!=='win32'});
        active.child=child;
        exited=new Promise((resolve,reject)=>{child.once('error',reject);child.once('close',(code)=>resolve(code));});
        // Avoid unhandled rejection while reading stdout after an executable-not-found error.
        exited.catch(()=>{});
        child.stderr.resume(); // Never surface raw provider output or credentials.
        child.stdin.on('error',()=>{});child.stdin.end(request);
        if(!prepare)this.event('turn_accepted',{turnId,contextIds});
        timer=setTimeout(()=>{active.timedOut=true;this.terminate(active);},prepare?60000:180000);
        for await(const line of createInterface({input:child.stdout,crlfDelay:Infinity})) {
          if(line.length>2000000)continue;
          let event;try{event=JSON.parse(line);}catch{continue;}
          if(active.cancelled)continue;
          if(event.session_id && event.session_id!==r.session)throw Error('Claude returned a different session.');
          if(event.type==='system'&&event.subtype==='init'){r.started=true;await this.save();this.event('session',{session:r.session,tools:event.tools||[]});}
          if(event.type==='stream_event') {
            const e=event.event;
            if(e?.type==='content_block_delta'&&e.delta?.type==='text_delta') {
              const text=String(e.delta.text||'');active.answer+=text;
              if(active.answer.length>150000)throw Error('Answer exceeded the display limit');
              if(!prepare)this.event('delta',{text});
            }
            if(e?.type==='content_block_start'&&e.content_block?.type==='tool_use')this.event('activity',{label:e.content_block.name?.includes('artifact_create')?'Saving artifact':e.content_block.name?.includes('artifact_')?'Reading saved artifacts':'Reading repository context',tool:e.content_block.name});
          }
          if(event.type==='result')active.result=event;
        }
        const code=await exited;
        if(active.cancelled)return;
        if(active.timedOut||code!==0||active.result?.is_error||!active.result)throw Error('Claude could not finish. Check claude auth status, model access, budget, or retry. If the native session was deleted, use New chat.');
        const text=String(active.result.result||active.answer);
        if(prepare)r.summary=text;else r.messages.push({role:'assistant',content:text});
        r.started=true;r.revision=revision(c.pr);
        await this.refreshEvidence(c,active);
        r.evidence=c.evidence;
        r.codeContext={revision:r.revision,providedPatchPaths:[...new Set([...known,...fresh.patches.map(p=>p.path)])],patchFingerprints:[...new Set([...fingerprints,...fresh.patches.map(fingerprint)])],readSources:[...new Map([...sources,...c.evidence.readSources].map(s=>[JSON.stringify(s),s])).values()].slice(-256)};
        await this.save();this.state('Ready');
        if(!prepare)this.event('turn_complete',{turnId,contextIds});
      } catch(error) {
        if(!active.cancelled){this.terminate(active);if(exited)await exited.catch(()=>{});this.state('Error',error.code==='ENOENT'?'Claude Code was not found. Install it or set ai.command.':String(error.message||'Claude could not finish.'));}
      } finally {
        clearTimeout(timer);clearInterval(poll);
        await this.refreshArtifacts(c);
        await this.refreshEvidence(c,active);
        if(active.cancelled&&!prepare&&active.answer)r.messages.push({role:'assistant',content:active.answer+'\n\n[Interrupted]'});
        if(active.timedOut)this.state('Error','Claude timed out. Retry or ask a narrower question.');
        await this.save().catch(()=>{});
        if(this.active===active)this.active=null;
      }
    })();
  }
  terminate(active) {
    const child=active.child;if(!child||child.exitCode!==null||child.signalCode!==null)return;
    const signal=s=>{try{if(process.platform==='win32')child.kill(s);else process.kill(-child.pid,s);}catch{}};
    signal('SIGINT');
    const force=setTimeout(()=>{signal('SIGKILL');},1500);force.unref();child.once('close',()=>{clearTimeout(force);signal('SIGKILL');});
  }
  async stop() {
    const active=this.active;if(!active)return;
    active.cancelled=true;this.terminate(active);await active.done;
    if(this.current)this.state('Stopped');
  }
  async close() {
    clearInterval(this.nativePoll);
    try{await this.pollNative();}
    finally{await this.stop();if(this.current){await this.current.release();this.current=null;}}
  }
}
if(process.argv[1]&&import.meta.url===pathToFileURL(realpathSync(process.argv[1])).href) {
  const companion=new Companion(value=>process.stdout.write(JSON.stringify(value)+'\n'));
  let chain=Promise.resolve();
  const input=createInterface({input:process.stdin,crlfDelay:Infinity});
  const handle=async line=>{
    let command;
    try {
      if(line.length>10000000)throw Error('PR context too large');
      command=JSON.parse(line);
      if(command.operation==='open')await companion.open(command);
      else if(command.operation==='terminal')await companion.terminal(command.binding);
      else if(command.operation==='native_context')await companion.nativeContext(command);
      else if(command.operation==='ask')await companion.send(command);
      else if(command.operation==='new')await companion.fresh(command.binding);
      else if(command.operation==='artifact_open')await companion.openOutput(command.binding,command.id);
      else if(command.operation==='stop')await companion.stop();
      else if(command.operation==='close')await companion.close();
      else if(command.operation==='retry'&&companion.current&&!companion.active)companion.run('',true);
    }catch(error){process.stdout.write(JSON.stringify({binding:command?.binding,type:'error',error:String(error.message||'Companion failed')})+'\n');}
  };
  for await(const line of input)chain=chain.then(()=>handle(line));
  await chain;await companion.close();
}
