// Real-browser showcase regression. Node 22+, installed Chrome/Chromium; no npm dependencies.
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {openBrowser,wait} from './browser-harness.mjs';
import {paintInstrumentation} from './browser-metrics.mjs';
const browser=await openBrowser(process.argv[2]);
const {cdp,evaluate,text,click,type,key,viewport,errors,requests}=browser;
let url=browser.url;
try{
  await cdp('Page.addScriptToEvaluateOnNewDocument',{source:paintInstrumentation});
  const homeUrl=url;
  await cdp('Page.navigate',{url:homeUrl});
  await wait(400);
  assert.equal(await evaluate('document.querySelector("#try-review").getAttribute("href")'),'demo/');
  assert.equal(await evaluate('document.querySelector(".sim")'),null);
  assert.equal(await evaluate('document.querySelector(".demo-poster").getAttribute("href")'),'demo/');
  const interactiveUrl=new URL('demo/',homeUrl).href;
  const until=async expression=>{for(let i=0;i<150;i++){if(await evaluate(expression))return;await wait(40);}throw Error('Native state did not settle: '+expression+' '+await evaluate('JSON.stringify(window.lazyreviewTui?.getState())'));};
  await cdp('Page.navigate',{url:interactiveUrl});
  await until('!!window.lazyreviewTui?.getState().ready');
  await evaluate('document.fonts.ready');
  await until('__paintStats.emptyFrames>=2'); // Observe settled syntax before measuring idle work.
  await evaluate('Object.keys(__paintStats).forEach(k=>__paintStats[k]=0)');await wait(2200);
  const idle=await evaluate('({...__paintStats})');
  assert.ok(idle.emptyFrames>=2,'Observe native idle responses');
  assert.equal(idle.glyphs,0,'Idle responses must not redraw the terminal');
  assert.equal(idle.rects,0,'Idle responses must not clear backgrounds');
  assert.equal(idle.transcripts,0,'Idle responses must not rewrite the accessible transcript');
  assert.equal(await evaluate('document.activeElement.id'),'input');
  assert.equal(await evaluate('window.LAZYREVIEW_TERMINAL_PROFILE.font.family'),'JetBrainsMono Nerd Font');
  assert.equal(await evaluate('window.LAZYREVIEW_TERMINAL_PROFILE.font.sizePoints'),16);
  assert.ok(await evaluate('document.fonts.check("32px LazyReviewTerminal")'));
  assert.equal(await evaluate('window.lazyreviewTui.getState().pane'),0);
  const cellPoint=async(x,y)=>evaluate(`(()=>{const r=document.querySelector('#screen').getBoundingClientRect();const p=window.LAZYREVIEW_TERMINAL_PROFILE.raster;return {x:r.left+(${x}+.5)*p.cellWidth/p.scale,y:r.top+(${y}+.5)*p.cellHeight/p.scale};})()`);
  const canvasClick=async(x,y)=>{const point=await cellPoint(x,y);await cdp('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...point});await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...point});};
  let area=await evaluate('window.lazyreviewTui.getState().areas[1]');await canvasClick(area[0]+5,area[1]+5);await until('window.lazyreviewTui.getState().pane===1');
  area=await evaluate('window.lazyreviewTui.getState().areas[0]');await canvasClick(area[0]+5,area[1]+5);await until('window.lazyreviewTui.getState().pane===0');
  const cursor=await evaluate('window.lazyreviewTui.getState().treeCursor');await key('j');await until(`window.lazyreviewTui.getState().treeCursor>${cursor}`);await key('k');await until(`window.lazyreviewTui.getState().treeCursor===${cursor}`);
  await key('n');await until('window.lazyreviewTui.getState().treeCursor>0');const file=await evaluate('window.lazyreviewTui.getState().file');await key('n');await until(`window.lazyreviewTui.getState().file!==${JSON.stringify(file)}`);await key('N',8);await until(`window.lazyreviewTui.getState().file===${JSON.stringify(file)}`);
  await key('2');await until('window.lazyreviewTui.getState().category==="Tests"');await key('1');await until('window.lazyreviewTui.getState().category==="Code"');
  await key('/');await until('window.lazyreviewTui.getState().editing');await type('#input','use-keyboard');await key('Enter');await until('!window.lazyreviewTui.getState().editing');assert.match(await evaluate('window.lazyreviewTui.text()'),/use-keyboard/);
  await key('/');await until('window.lazyreviewTui.getState().editing');await key('Escape');await until('!window.lazyreviewTui.getState().editing');
  await click('#reset');await until(`window.lazyreviewTui.getState().ready&&window.lazyreviewTui.getState().category==='Code'`);
  await key('Tab');await until('window.lazyreviewTui.getState().pane===1');await key('s');await until('window.lazyreviewTui.getState().split');await key('s');await until('!window.lazyreviewTui.getState().split');
  await key('v');await key('j');await key('t');await until('window.lazyreviewTui.getState().modal==="Editor(AiNote)"');await type('#input','Why is this boundary here?');await key('Enter');await until('window.lazyreviewTui.getState().context===1');
  await key('n');await key('v');await key('j');await key('t');await until('window.lazyreviewTui.getState().modal==="Editor(AiNote)"');await type('#input','How does the other file fit?');await key('Enter');await until('window.lazyreviewTui.getState().context===2');
  await key('g');await key('c');await until('window.lazyreviewTui.getState().modal==="Context"');assert.match(await evaluate('window.lazyreviewTui.text()'),/How does the other file/);await key('k');await until('window.lazyreviewTui.text().includes("Why is this boundary")');await key('Escape');
  await key('g');await key('a');await until('window.lazyreviewTui.getState().panelVisible');await key('i');await until('window.lazyreviewTui.getState().modal==="Editor(Chat)"');await type('#input','Explain how these selections work together');await key('Enter');await until('window.lazyreviewTui.getState().messages===2&&window.lazyreviewTui.getState().context===0');assert.match(await evaluate('window.lazyreviewTui.text()'),/SAMPLE EXPLANATION/);
  const sampleRow=await evaluate('window.lazyreviewTui.text().split("\\n").findIndex(line=>line.includes("SAMPLE EXPLANATION"))');
  area=await evaluate('window.lazyreviewTui.getState().readerArea');
  const sampleColumn=await evaluate('window.lazyreviewTui.text().split("\\n").find(line=>line.includes("SAMPLE EXPLANATION")).indexOf("SAMPLE")');
  const dragStart=await cellPoint(sampleColumn,sampleRow),dragEnd=await cellPoint(sampleColumn+20,sampleRow);
  await cdp('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...dragStart});await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',button:'left',buttons:1,...dragEnd});await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...dragEnd});
  await until('window.lazyreviewTui.getState().selectionText.length>0');assert.match(await evaluate('window.lazyreviewTui.getState().selectionText'),/SAMPLE/);
  await key('Tab',8);await until('window.lazyreviewTui.getState().pane===1');await key('v');await key('j');await key('c');await until('window.lazyreviewTui.getState().modal==="Editor(Comment)"');await type('#input','A browser-only sample draft');await key('Enter');await until('window.lazyreviewTui.getState().drafts===1');
  await key('r');await until('window.lazyreviewTui.getState().panel==="Review"');assert.match(await evaluate('window.lazyreviewTui.text()'),/review/i);
  await key('S',8);await until('window.lazyreviewTui.getState().modal==="Editor(Review)"');await type('#input','Sample review, no GitHub write');await key('Tab');await key('Enter');await until('window.lazyreviewTui.getState().modal==="Confirm"');await key('Enter');await until('!window.lazyreviewTui.getState().modal&&window.lazyreviewTui.getState().drafts===0&&window.lazyreviewTui.getState().posted>0');
  await click('#help');await until('window.lazyreviewTui.getState().modal==="Help"');await key('Escape');await until('!window.lazyreviewTui.getState().modal');
  await key(']',2,'BracketRight');assert.equal(await evaluate('document.activeElement.id'),'help');
  await mkdir(new URL('../outputs/',import.meta.url),{recursive:true});
  for(const width of [1440,768,390]){await viewport(width,width<600?844:900);await wait(500);assert.ok(await evaluate('document.documentElement.scrollWidth<=innerWidth'));assert.ok(await evaluate('document.documentElement.scrollHeight<=innerHeight+1'));const shot=await cdp('Page.captureScreenshot',{format:'png'});await writeFile(new URL(`../outputs/interactive-tui-${width}.png`,import.meta.url),Buffer.from(shot.data,'base64'));}
  await viewport(1440,900);await wait(500);
  // A slow response from a replaced session must not block input in the new one.
  const replacedSession=await evaluate('window.lazyreviewTui.getState().session');
  await evaluate(`(()=>{const realFetch=window.fetch;let held=false;window.fetch=async(url,options)=>{if(JSON.parse(options.body).events&&!held){held=true;const response=await realFetch(url,options);await new Promise(resolve=>window.__releaseStale=resolve);return response;}return realFetch(url,options);};window.__restoreFetch=()=>window.fetch=realFetch;document.querySelector('#input').focus();})()`);
  await key('j');await until('typeof window.__releaseStale==="function"');await click('#reset');
  await until(`window.lazyreviewTui.getState().ready&&window.lazyreviewTui.getState().session!==${JSON.stringify(replacedSession)}`);
  await key('j');await until('window.lazyreviewTui.getState().treeCursor>0');
  await evaluate('__restoreFetch();__releaseStale()');await wait(50);
  assert.notEqual(await evaluate('window.lazyreviewTui.getState().session'),replacedSession);
  // A dead session stops polling and input until an explicit reset creates a new one.
  await evaluate(`(()=>{const realFetch=window.fetch;window.__expiredRequests=0;window.fetch=(url,options)=>{const body=JSON.parse(options.body);if(body.events){window.__expiredRequests++;return Promise.resolve(new Response(JSON.stringify({error:'Session expired'}),{status:410,headers:{'content-type':'application/json'}}));}return realFetch(url,options);};window.__restoreFetch=()=>window.fetch=realFetch;document.querySelector('#input').focus();})()`);
  await key('j');await until('!document.querySelector("#failure").hidden');
  const expiredRequests=await evaluate('__expiredRequests');await wait(1300);await key('j');await wait(100);
  assert.equal(await evaluate('__expiredRequests'),expiredRequests,'Failed sessions must stop polling and input');
  assert.equal(await evaluate('window.lazyreviewTui.getState().ready'),false);
  assert.equal(await evaluate('document.activeElement.id'),'retry');
  await evaluate('__restoreFetch()');await key('Enter');await until('window.lazyreviewTui.getState().ready');
  await click('#help');await until('window.lazyreviewTui.getState().modal==="Help"');await key('Escape');
  const previousSession=await evaluate('window.lazyreviewTui.getState().session');
  await evaluate(`dispatchEvent(new PageTransitionEvent('pagehide',{persisted:true}));dispatchEvent(new PageTransitionEvent('pageshow',{persisted:true}));`);
  await until(`window.lazyreviewTui.getState().ready&&window.lazyreviewTui.getState().session!==${JSON.stringify(previousSession)}`);
  await key('j');await until('window.lazyreviewTui.getState().treeCursor>0');
  const nativeUrl=new URL('demo/tour/',homeUrl).href;
  await cdp('Page.navigate',{url:nativeUrl});
  for(let i=0;i<100;i++){if(await evaluate('!!window.reviewDemo && window.reviewDemo.getState().imageReady'))break;await wait(50);}
  assert.ok(await evaluate('window.reviewDemo.getState().imageReady'));
  assert.equal(await evaluate('window.LR_TUI_DATA.terminalProfile.font.family'),'JetBrainsMono Nerd Font');
  assert.equal(await evaluate('window.LR_TUI_DATA.terminalProfile.font.sizePoints'),16);
  assert.equal(await evaluate('window.LR_TUI_DATA.terminalProfile.window.opacity'),0.7);
  assert.equal(await evaluate('getComputedStyle(document.querySelector(".viewport")).backgroundColor'),'rgb(9, 3, 0)');
  assert.equal(await evaluate('document.querySelector(".window-dots")'),null);
  assert.equal(await evaluate('window.reviewDemo.getState().playing'),false);
  await evaluate('window.reviewDemo.seek(4.2)');await wait(100);
  assert.equal(await evaluate('[...document.querySelectorAll("#key-sequence kbd")].length'),0);
  await click('#play');await wait(120);assert.equal(await evaluate('window.reviewDemo.getState().playing'),true);await click('#play');
  await click('#next');assert.equal(await evaluate('window.reviewDemo.getState().index'),2);
  await evaluate('window.reviewDemo.seek(9.7)');
  assert.deepEqual(await evaluate('[...document.querySelectorAll("#key-sequence kbd")].map(e=>e.textContent)'),['2']);
  await click('#actual-size');
  assert.equal(await evaluate('window.reviewDemo.getState().actualSize'),true);
  assert.equal(await evaluate('document.querySelector("#terminal-image").getBoundingClientRect().width'),1710);
  assert.ok(await evaluate('document.querySelector(".viewport").scrollWidth>document.querySelector(".viewport").clientWidth'));
  await click('#actual-size');
  await mkdir(new URL('../outputs/',import.meta.url),{recursive:true});
  for(const width of [1440,768,390]){
    await viewport(width,width<600?844:1000);await evaluate('window.scrollTo({top:0,behavior:"instant"});window.reviewDemo.seek(74)');await wait(400);
    assert.ok(await evaluate('document.documentElement.scrollWidth<=innerWidth'),`Native demo overflows at ${width}`);
    assert.ok(await evaluate('window.reviewDemo.getState().imageReady'));
    const shot=await cdp('Page.captureScreenshot',{format:'png'});await writeFile(new URL(`../outputs/tui-${width}.png`,import.meta.url),Buffer.from(shot.data,'base64'));
  }
  await cdp('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
  await evaluate('window.reviewDemo.seek(30)');assert.equal(await evaluate('document.querySelector("#camera").style.transition'),'none');
  await cdp('Emulation.setEmulatedMedia',{features:[]});await viewport(1440,900);
  url=new URL('demo/web/',homeUrl).href;
  await cdp('Page.navigate',{url});
  for(let i=0;i<100;i++){if(await evaluate('!!window.LR_SAMPLE && document.querySelector(".sim")?.hidden===false'))break;await wait(50);}
  assert.equal(await evaluate('document.querySelector(".sim").hidden'),false);
  await evaluate('document.fonts.ready');await wait(800);
  assert.equal(await evaluate('getComputedStyle(document.querySelector(".fallback-workspace")).display'),'none');
  assert.equal(await evaluate('document.querySelectorAll("[data-file]").length'),18);
  await click('[data-filter="Docs"]');assert.equal(await evaluate('document.querySelectorAll("[data-file]").length'),3);assert.match(await text('#current-file'),/\.md$/);
  await click('[data-filter="Code"]');await type('#file-search','kybrdnvgtn');assert.equal(await evaluate('document.querySelectorAll("[data-file]").length'),1);
  await click('#reset-demo');
  await evaluate('document.querySelector("#diff").focus()');await key('h');await key('j');const afterJ=await text('#current-file');await key('j');assert.notEqual(await text('#current-file'),afterJ);assert.ok(await evaluate('!!document.activeElement.closest(".file-pane")'));
  await click('#reset-demo');await click('[data-line="1"]');await key('j');assert.equal(await evaluate('document.activeElement.id'),'diff');
  await click('[data-line="1"]');
  await evaluate(`document.querySelector('[data-line="4"]').dispatchEvent(new MouseEvent("click",{bubbles:true,shiftKey:true}))`);
  assert.match(await text('#selection-label'),/\d+ lines selected/);
  await click('#note-toggle');await type('#note-body','How do these results stay in sync?');await click('#note-form button');assert.equal(await text('#context-count'),'1');
  await click('[data-file="1"]');await click('[data-line="1"]');await click('#note-toggle');await type('#note-body','<img src=x onerror="window.__pwn=1">');await click('#note-form button');assert.equal(await text('#context-count'),'2');
  await click('#context-toggle');assert.equal(await evaluate('document.querySelectorAll(".context-note").length'),2);assert.match(await text('#dialog-body'),/<img src=x/);assert.equal(await evaluate('window.__pwn'),undefined);await key('Escape');assert.equal(await evaluate('document.querySelector("dialog").open'),false);
  await type('#chat-input','Explain how these files interact');await click('#chat-form button');assert.equal(await text('#context-count'),'0');assert.match(await text('#chat'),/Sample context/);assert.match(await text('#chat'),/Scripted demo reply/);
  await type('#chat-input','A general PR question');await key(']',2,'BracketRight');assert.equal(await evaluate('document.activeElement.id'),'diff');
  await click('#viewed-toggle');assert.match(await text('#viewed-count'),/^1 \/ 18/);
  await click('#layout-toggle');assert.equal(await evaluate('document.querySelectorAll(".split-half").length'),2);await click('#layout-toggle');
  await click('[data-line="1"]');await click('#comment-toggle');await type('#note-body','A sample code comment');await click('#note-form button');await click('#review-toggle');assert.equal(await evaluate('document.querySelector("#dialog-body details").open'),false);await click('#dialog-body summary');assert.match(await text('#dialog-body'),/A sample code comment/);await click('[data-outcome="Approve"]');assert.match(await text('#dialog-body'),/no GitHub request/);await key('Escape');
  await evaluate('document.querySelector(".wordmark").focus()');await key('r');assert.equal(await evaluate('document.querySelector("dialog").open'),false);
  await evaluate('document.querySelector("#diff").focus()');await key('g');await key('a');assert.equal(await evaluate('document.activeElement.id'),'chat-input');await key(']',2,'BracketRight');assert.equal(await evaluate('document.activeElement.id'),'diff');
  await mkdir(new URL('../outputs/',import.meta.url),{recursive:true});
  await wait(3300); // Let transient draft feedback clear before photographing the workspace.
  for(const width of [1440,768,390]){
    await viewport(width,width<600?844:900);await evaluate('window.scrollTo({top:0,behavior:"instant"})');await wait(400);
    assert.ok(await evaluate('document.documentElement.scrollWidth<=innerWidth'),`Page overflows at ${width}`);
    assert.ok(await evaluate('document.documentElement.scrollHeight<=innerHeight+1'),`Demo is not full-viewport at ${width}`);
    const shot=await cdp('Page.captureScreenshot',{format:'png'});await writeFile(new URL(`../outputs/demo-${width}.png`,import.meta.url),Buffer.from(shot.data,'base64'));
  }
  await click('[data-pane="files"]');await click('[data-filter="Tests"]');await click('[data-file]');assert.equal(await evaluate('document.querySelector(".sim").dataset.mobilePane'),'changes');await click('[data-line="1"]');await click('#note-toggle');await type('#note-body','Mobile question');await click('#note-form button');await click('[data-pane="companion"]');assert.equal(await text('#context-count'),'1');await type('#chat-input','Explain the tests');await click('#chat-form button');assert.equal(await text('#context-count'),'0');
  await click('#reset-demo');assert.equal(await text('#draft-count'),'0');assert.equal(await text('#viewed-count'),'0 / 18 viewed');
  for(const preset of ['?filter=Code','?select=1']){await cdp('Page.navigate',{url:url+preset});await wait(350);if(preset.includes('filter'))assert.ok(await evaluate('[...document.querySelectorAll("[data-file]")].every(b=>window.LR_SAMPLE.files[+b.dataset.file].category==="Code")'));else assert.equal(await evaluate('document.querySelector("#note-toggle").disabled'),false);}
  await cdp('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});assert.equal(await evaluate('getComputedStyle(document.querySelector("h1")).opacity'),'1');
  await cdp('Emulation.setScriptExecutionDisabled',{value:true});await cdp('Page.navigate',{url});await wait(500);
  assert.notEqual(await evaluate('getComputedStyle(document.querySelector(".fallback-workspace")).display'),'none');
  assert.equal(await evaluate('document.querySelector(".sim").hidden'),true);
  assert.ok(await evaluate('document.querySelector(".fallback-workspace").naturalWidth>0'));
  assert.deepEqual(errors,[]);assert.ok(requests.every(u=>u.startsWith(new URL(url).origin)||u.startsWith('data:')),'Unexpected external request');
  console.log('PASS interactive native /demo/: real Rust handlers, Vim navigation, file filtering, multi-file notes, scripted chat, drafts, mouse help and responsive resizing. PASS recorded /demo/tour/: pinned display profile, authentic cell images, playback, KeyCastr, actual-size pan, reduced motion and responsive layouts. PASS preserved /demo/web/: real Chrome, showcase links, 18 files, direct entry, full viewport, keyboard focus, filters, multi-file notes, escaping, scripted replies, Ctrl-], drafts, keyboard scope, mobile, reduced motion, no-JS fallback; no external requests.');
 }finally{await browser.close();}
