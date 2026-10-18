// Real-browser showcase regression. Node 22+, installed Chrome/Chromium; no npm dependencies.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {existsSync} from 'node:fs';
import {mkdtemp,mkdir,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
const chrome=process.env.CHROME_PATH||['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome','/usr/bin/google-chrome','/usr/bin/chromium','/usr/bin/chromium-browser'].find(existsSync);
if(!chrome)throw Error('Install Chrome/Chromium or set CHROME_PATH');
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const profile=await mkdtemp(join(tmpdir(),'lazyreview-site-'));
const root=new URL('../',import.meta.url);
let browser,server,ws,serial=0;
const pending=new Map(),errors=[],requests=[];
function output(child,stream,pattern){return new Promise((resolve,reject)=>{let text='';const timer=setTimeout(()=>reject(Error('Process startup timed out: '+text.slice(-500))),20000);child[stream].on('data',d=>{text+=d;const m=text.match(pattern);if(m){clearTimeout(timer);resolve(m[1]);}});child.once('error',e=>{clearTimeout(timer);reject(e);});child.once('exit',code=>{clearTimeout(timer);reject(Error('Process exited '+code+': '+text.slice(-500)));});});}
function send(method,params={},sessionId){return new Promise((resolve,reject)=>{const id=++serial;const timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout '+method));},20000);pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({id,method,params,...(sessionId?{sessionId}:{})}));});}
try{
  let url=process.argv[2];
  if(!url){server=spawn(process.execPath,['scripts/serve-site.mjs'],{cwd:root,env:{...process.env,PORT:'0'},stdio:['ignore','pipe','pipe']});
  url=await output(server,'stdout',/(http:\/\/127\.0\.0\.1:\d+\/lazyreview\/)/);}
  browser=spawn(chrome,['--headless=new','--remote-debugging-port=0',`--user-data-dir=${profile}`,'--no-first-run','--no-default-browser-check','--disable-background-networking','--disable-features=Translate,MediaRouter','--hide-scrollbars','about:blank'],{stdio:['ignore','ignore','pipe']});
  const endpoint=await output(browser,'stderr',/DevTools listening on (ws:\/\/[^\s]+)/);
  ws=new WebSocket(endpoint);await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
  ws.onmessage=event=>{const msg=JSON.parse(event.data);if(msg.id){const p=pending.get(msg.id);if(!p)return;clearTimeout(p.timer);pending.delete(msg.id);msg.error?p.reject(Error(msg.error.message)):p.resolve(msg.result);}else{if(msg.method==='Runtime.exceptionThrown')errors.push(msg.params.exceptionDetails.text);if(msg.method==='Network.requestWillBeSent')requests.push(msg.params.request.url);}};
  const {targetId}=await send('Target.createTarget',{url:'about:blank'});
  const {sessionId}=await send('Target.attachToTarget',{targetId,flatten:true});
  const cdp=(method,params)=>send(method,params,sessionId);
  const evaluate=async(expression)=>{const r=await cdp('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;};
  const text=selector=>evaluate(`document.querySelector(${JSON.stringify(selector)})?.textContent`);
  const click=async selector=>{const point=await evaluate(`(()=>{let e=document.querySelector(${JSON.stringify(selector)});if(!e)throw Error('Missing '+${JSON.stringify(selector)});e.scrollIntoView({block:'center',behavior:'instant'});let r=e.getBoundingClientRect();return {x:Math.max(0,r.left)+Math.min(15,r.width/2),y:r.top+r.height/2};})()`);await cdp('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...point});await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...point});};
  const type=async(selector,value)=>{await evaluate(`document.querySelector(${JSON.stringify(selector)}).focus()`);await cdp('Input.insertText',{text:value});};
  const key=async(key,modifiers=0,code)=>{const vk=({Escape:27,Enter:13,Tab:9,']':221})[key]||key.toUpperCase().charCodeAt(0);const props={key,modifiers,windowsVirtualKeyCode:vk,nativeVirtualKeyCode:vk,...(code?{code}:{})};await cdp('Input.dispatchKeyEvent',{type:'rawKeyDown',...props});await cdp('Input.dispatchKeyEvent',{type:'keyUp',...props});await wait(30);};
  const viewport=async(width,height)=>{await cdp('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:width<600});};
  await cdp('Page.enable');await cdp('Runtime.enable');await cdp('Network.enable');await viewport(1440,900);
  await cdp('Page.navigate',{url});
  for(let i=0;i<100;i++){if(await evaluate('!!window.LR_SAMPLE && document.querySelector(".sim")?.hidden===false'))break;await wait(50);}
  assert.equal(await evaluate('document.querySelector(".sim").hidden'),false);
  await evaluate('document.fonts.ready');await wait(800);
  assert.equal(await evaluate('getComputedStyle(document.querySelector(".fallback-workspace")).display'),'none');
  assert.equal(await evaluate('document.querySelectorAll("[data-file]").length'),4);
  await click('[data-filter="Docs"]');assert.equal(await evaluate('document.querySelectorAll("[data-file]").length'),1);assert.match(await text('#current-file'),/\.md$/);
  await click('[data-filter="All"]');await type('#file-search','kybrdnvgtn');assert.equal(await evaluate('document.querySelectorAll("[data-file]").length'),1);
  await click('#reset-demo');
  await click('[data-line="1"]');
  await evaluate(`document.querySelector('[data-line="4"]').dispatchEvent(new MouseEvent("click",{bubbles:true,shiftKey:true}))`);
  assert.match(await text('#selection-label'),/\d+ lines selected/);
  await click('#note-toggle');await type('#note-body','How do these results stay in sync?');await click('#note-form button');assert.equal(await text('#context-count'),'1');
  await click('[data-file="1"]');await click('[data-line="1"]');await click('#note-toggle');await type('#note-body','<img src=x onerror="window.__pwn=1">');await click('#note-form button');assert.equal(await text('#context-count'),'2');
  await click('#context-toggle');assert.equal(await evaluate('document.querySelectorAll(".context-note").length'),2);assert.match(await text('#dialog-body'),/<img src=x/);assert.equal(await evaluate('window.__pwn'),undefined);await key('Escape');assert.equal(await evaluate('document.querySelector("dialog").open'),false);
  await type('#chat-input','Explain how these files interact');await click('#chat-form button');assert.equal(await text('#context-count'),'0');assert.match(await text('#chat'),/Sample context/);assert.match(await text('#chat'),/Scripted demo reply/);
  await type('#chat-input','A general PR question');await key(']',2,'BracketRight');assert.equal(await evaluate('document.activeElement.id'),'diff');
  await click('#viewed-toggle');assert.match(await text('#viewed-count'),/^1 \/ 4/);
  await click('#layout-toggle');assert.equal(await evaluate('document.querySelectorAll(".split-half").length'),2);await click('#layout-toggle');
  await click('[data-line="1"]');await evaluate('document.querySelector("#diff").focus()');await key('c');await type('#note-body','A sample code comment');await click('#note-form button');await click('#review-toggle');assert.equal(await evaluate('document.querySelector("#dialog-body details").open'),false);await click('#dialog-body summary');assert.match(await text('#dialog-body'),/A sample code comment/);await click('[data-outcome="Approve"]');assert.match(await text('#dialog-body'),/no GitHub request/);await key('Escape');
  await evaluate('document.querySelector(".wordmark").focus()');await key('r');assert.equal(await evaluate('document.querySelector("dialog").open'),false);
  await evaluate('document.querySelector("#diff").focus()');await key('g');await key('a');assert.equal(await evaluate('document.activeElement.id'),'chat-input');await key(']',2,'BracketRight');assert.equal(await evaluate('document.activeElement.id'),'diff');
  await mkdir(new URL('../outputs/',import.meta.url),{recursive:true});
  for(const width of [1440,768,390]){
    await viewport(width,width<600?844:900);await evaluate('window.scrollTo({top:0,behavior:"instant"})');await wait(400);
    assert.ok(await evaluate('document.documentElement.scrollWidth<=innerWidth'),`Page overflows at ${width}`);
    const shot=await cdp('Page.captureScreenshot',{format:'png'});await writeFile(new URL(`../outputs/site-${width}.png`,import.meta.url),Buffer.from(shot.data,'base64'));
  }
  await click('[data-pane="files"]');await click('[data-filter="Tests"]');await click('[data-file]');assert.equal(await evaluate('document.querySelector(".sim").dataset.mobilePane'),'changes');await click('[data-line="1"]');await click('#note-toggle');await type('#note-body','Mobile question');await click('#note-form button');await click('[data-pane="companion"]');assert.equal(await text('#context-count'),'1');await type('#chat-input','Explain the tests');await click('#chat-form button');assert.equal(await text('#context-count'),'0');
  await click('#reset-demo');assert.equal(await text('#draft-count'),'0');assert.equal(await text('#viewed-count'),'0 / 4 viewed');
  await cdp('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});assert.equal(await evaluate('getComputedStyle(document.querySelector("h1")).opacity'),'1');
  await cdp('Emulation.setScriptExecutionDisabled',{value:true});await cdp('Page.navigate',{url});await wait(500);
  assert.notEqual(await evaluate('getComputedStyle(document.querySelector(".fallback-workspace")).display'),'none');
  assert.equal(await evaluate('document.querySelector(".sim").hidden'),true);
  assert.ok(await evaluate('document.querySelector(".fallback-workspace").naturalWidth>0'));
  assert.deepEqual(errors,[]);assert.ok(requests.every(u=>u.startsWith(new URL(url).origin)||u.startsWith('data:')),'Unexpected external request');
  console.log('PASS showcase: real Chrome, filters, multi-file notes, escaping, scripted replies, Ctrl-], drafts, keyboard scope, mobile, reduced motion, no-JS fallback; no external requests.');
}finally{
  if(ws?.readyState===WebSocket.OPEN){try{await send('Browser.close');}catch{}ws.close();}
  browser?.kill();server?.kill();await wait(250);await rm(profile,{recursive:true,force:true});
}
