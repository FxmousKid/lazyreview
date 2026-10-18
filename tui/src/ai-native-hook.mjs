// Public provider lifecycle hooks, never private transcript parsing.
import {readFile,mkdir,writeFile,rename} from 'node:fs/promises';
import {randomUUID,createHash} from 'node:crypto';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {realpathSync} from 'node:fs';
import {briefing,patchEvidence,questionContext,currentAttachments,revision} from './ai-context.mjs';
async function save(path,value){const tmp=path+'.'+randomUUID()+'.tmp';await writeFile(tmp,JSON.stringify(value),{mode:0o600});await rename(tmp,path);}
export async function hook(event,dir,launch){
  const pr=JSON.parse(await readFile(join(dir,'snapshot.json'),'utf8'));
  event={...event,session_id:event.session_id||event.sessionId,turn_id:event.turn_id||event.promptId,
    last_assistant_message:event.last_assistant_message??event.lastAssistantMessage};
  const kind=event.hook_event_name||({session_start:'SessionStart',user_prompt_submit:'UserPromptSubmit',stop:'Stop',stop_failure:'StopFailure',stop_cancelled:'StopCancelled'}[event.hookEventName]);
  let active={session:pr.chatSession};try{active=JSON.parse(await readFile(join(dir,'native-active.json'),'utf8'));}catch{}
  if(launch&&active.launch!==launch)return {};
  const changed=kind==='SessionStart'&&event.session_id!==active.session;
  if(changed&&!['clear','resume','startup','new','load','fork','compact'].includes(event.source))return {};
  if(!/^[a-f0-9-]{36}$/i.test(event.session_id||''))return {};
  if(!changed&&event.session_id!==active.session)return {};
  const id=randomUUID(),receipt={id,kind,session:event.session_id,launch:active.launch,time:Date.now()};
  if(changed){
    receipt.previousSession=active.session;receipt.source=event.source;
    await save(join(dir,'native-active.json'),{...active,previousSession:active.session,session:event.session_id});
    await save(join(dir,'snapshot.json'),{...pr,chatSession:event.session_id});
  }
  let output={};
  if(kind==='UserPromptSubmit'){
    const items=currentAttachments(pr,pr.pendingContext||[]),packet=patchEvidence(pr,event.prompt,pr.focus,items);
    let previous={};try{previous=JSON.parse(await readFile(join(dir,'native-code.json'),'utf8'));}catch{}
    if(previous.revision!==revision(pr)||previous.session!==event.session_id)previous={};
    const fingerprint=p=>createHash('sha256').update(p.path+'\0'+p.patch).digest('hex');
    const known=new Set(previous.fingerprints||[]);
    const fresh={...packet,patches:packet.patches.filter(p=>!known.has(fingerprint(p)))};
    const context=(previous.revision?`Current comparison ${revision(pr)}. Previously supplied code remains in this native session. Additional patches: ${JSON.stringify(fresh.patches)}`:briefing(pr,null,fresh))+`\n${packet.omittedPatchCount} patches outside this packet; use changes/read_file before making unsupported claims.\n`+questionContext(pr,pr.focus,items);
    const turn={id,providerTurnId:event.turn_id,launch:active.launch,delivered:active.provider!=='grok',session:event.session_id,prompt:String(event.prompt||'').slice(0,10000),items,revision:revision(pr),fingerprints:[...new Set([...known,...fresh.patches.map(fingerprint)])]};
    await save(join(dir,'native-turn.json'),turn);
    if(active.provider!=='grok')output={hookSpecificOutput:{hookEventName:kind,additionalContext:context}};
  } else if(kind==='Stop'){
    let turn;try{turn=JSON.parse(await readFile(join(dir,'native-turn.json'),'utf8'));}catch{}
    if(turn?.session===event.session_id&&!turn.cancelled&&(!event.turn_id||turn.providerTurnId===event.turn_id)){receipt.turn={...turn,items:turn.delivered?turn.items:[]};receipt.answer=String(event.last_assistant_message||'').slice(0,150000);await save(join(dir,'native-code.json'),{session:event.session_id,revision:turn.revision,fingerprints:turn.fingerprints});}
  } else if(['StopFailure','StopCancelled','Interrupt'].includes(kind)) {
    let turn;try{turn=JSON.parse(await readFile(join(dir,'native-turn.json'),'utf8'));}catch{}
    if(turn?.session===event.session_id&&(!event.turn_id||turn.providerTurnId===event.turn_id))await save(join(dir,'native-turn.json'),{...turn,cancelled:true});
  }
  // Only our fixed launch configuration can choose this path.
  const events=join(dir,'native-events');await mkdir(events,{recursive:true,mode:0o700});
  await save(join(events,`${receipt.time}-${id}.json`),receipt);
  return output;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(realpathSync(process.argv[1])).href){
  try {let input='';for await(const data of process.stdin){input+=data;if(input.length>2000000)throw Error('Hook too large');}
    const result=await hook(JSON.parse(input),process.argv[2],process.argv[3]||process.env.LAZYREVIEW_AI_LAUNCH);process.stdout.write(JSON.stringify(result));
  } catch {process.stderr.write('LazyReview context unavailable; queued notes have been retained.');process.exitCode=2;}
}
