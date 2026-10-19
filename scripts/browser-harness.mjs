// Isolated real-Chrome driver shared by the regression workflow and paint benchmark.
import {spawn} from 'node:child_process';
import {existsSync} from 'node:fs';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
export const wait=ms=>new Promise(r=>setTimeout(r,ms));
export async function openBrowser(url){
const chrome=process.env.CHROME_PATH||['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome','/usr/bin/google-chrome','/usr/bin/chromium','/usr/bin/chromium-browser'].find(existsSync);
if(!chrome)throw Error('Install Chrome/Chromium or set CHROME_PATH');
const profile=await mkdtemp(join(tmpdir(),'lazyreview-site-'));
const root=new URL('../',import.meta.url);
let browser,server,ws,serial=0;
const pending=new Map(),errors=[],requests=[];
function output(child,stream,pattern){return new Promise((resolve,reject)=>{let text='';const timer=setTimeout(()=>reject(Error('Process startup timed out: '+text.slice(-500))),20000);child[stream].on('data',d=>{text+=d;const m=text.match(pattern);if(m){clearTimeout(timer);resolve(m[1]);}});child.once('error',e=>{clearTimeout(timer);reject(e);});child.once('exit',code=>{clearTimeout(timer);reject(Error('Process exited '+code+': '+text.slice(-500)));});});}
function send(method,params={},sessionId){return new Promise((resolve,reject)=>{const id=++serial;const timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout '+method));},20000);pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({id,method,params,...(sessionId?{sessionId}:{})}));});}
async function stop(child){
  if(!child?.pid||child.exitCode!==null||child.signalCode!==null)return;
  await new Promise(resolve=>{
    const timer=setTimeout(()=>child.kill('SIGKILL'),3000);timer.unref();
    child.once('exit',()=>{clearTimeout(timer);resolve();});child.kill();
  });
}
async function close(){
  if(ws?.readyState===WebSocket.OPEN){try{await send('Browser.close');}catch{}ws.close();}
  await Promise.all([stop(browser),stop(server)]);await rm(profile,{recursive:true,force:true});
}
try{
  if(!url){server=spawn(process.execPath,['scripts/serve-site.mjs'],{cwd:root,env:{...process.env,PORT:'0'},stdio:['ignore','pipe','pipe']});
  url=await output(server,'stdout',/(http:\/\/127\.0\.0\.1:\d+\/lazyreview\/)/);}
  browser=spawn(chrome,['--headless=new','--remote-debugging-port=0',`--user-data-dir=${profile}`,'--no-first-run','--no-default-browser-check','--disable-background-networking','--disable-features=Translate,MediaRouter','--hide-scrollbars','about:blank'],{stdio:['ignore','ignore','pipe']});
  const endpoint=await output(browser,'stderr',/DevTools listening on (ws:\/\/[^\s]+)/);
  ws=new WebSocket(endpoint);await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
  ws.onclose=()=>{for(const p of pending.values()){clearTimeout(p.timer);p.reject(Error('Chrome connection closed'));}pending.clear();};
  ws.onmessage=event=>{const msg=JSON.parse(event.data);if(msg.id){const p=pending.get(msg.id);if(!p)return;clearTimeout(p.timer);pending.delete(msg.id);msg.error?p.reject(Error(msg.error.message)):p.resolve(msg.result);}else{if(msg.method==='Runtime.exceptionThrown')errors.push(msg.params.exceptionDetails.text);if(msg.method==='Network.requestWillBeSent')requests.push(msg.params.request.url);}};
  const {targetId}=await send('Target.createTarget',{url:'about:blank'});
  const {sessionId}=await send('Target.attachToTarget',{targetId,flatten:true});
  const cdp=(method,params)=>send(method,params,sessionId);
  const evaluate=async(expression)=>{const r=await cdp('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;};
  const text=selector=>evaluate(`document.querySelector(${JSON.stringify(selector)})?.textContent`);
  const click=async selector=>{const point=await evaluate(`(()=>{let e=document.querySelector(${JSON.stringify(selector)});if(!e)throw Error('Missing '+${JSON.stringify(selector)});e.scrollIntoView({block:'center',behavior:'instant'});let r=e.getBoundingClientRect();return {x:Math.max(0,r.left)+Math.min(15,r.width/2),y:r.top+r.height/2};})()`);await cdp('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...point});await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...point});};
  const type=async(selector,value)=>{await evaluate(`document.querySelector(${JSON.stringify(selector)}).focus()`);await cdp('Input.insertText',{text:value});};
  const key=async(key,modifiers=0,code)=>{const vk=({Escape:27,Enter:13,Tab:9,']':221})[key]||key.toUpperCase().charCodeAt(0);const props={key,modifiers,windowsVirtualKeyCode:vk,nativeVirtualKeyCode:vk,...(code?{code}:{})};await cdp('Input.dispatchKeyEvent',{type:key==='Enter'?'keyDown':'rawKeyDown',...props,...(key==='Enter'?{text:'\r',unmodifiedText:'\r'}:{})});await cdp('Input.dispatchKeyEvent',{type:'keyUp',...props});await wait(30);};
  const viewport=async(width,height)=>{await cdp('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:width<600});};
  await cdp('Page.enable');await cdp('Runtime.enable');await cdp('Network.enable');await viewport(1440,900);
  return {url,cdp,evaluate,text,click,type,key,viewport,errors,requests,close};
}catch(error){await close();throw error;}
}
