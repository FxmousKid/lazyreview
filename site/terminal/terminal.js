(() => {
 'use strict';
 const $=id=>document.getElementById(id),p=window.LAZYREVIEW_TERMINAL_PROFILE;
 const {cellWidth:cw,cellHeight:ch,fontSize,baseline,scale}=p.raster;
 const canvas=$('screen'),ctx=canvas.getContext('2d',{alpha:false}),input=$('input'),stage=$('terminal-scroll');
 let session=null,grid=[],cols=0,rows=0,state={},busy=false,queue=[],generation=0,copy='',keyTimer,pollTimer,resizeTimer,flushTimer,closed=false;
 const api=new URL('api',location.href).pathname;
 const colors=new Map();
 function background(hex){if(!colors.has(hex)){const blend=(a,b)=>Math.round(parseInt(a,16)*p.window.opacity+parseInt(b,16)*(1-p.window.opacity));const parts=[1,3,5].map(i=>blend(hex.slice(i,i+2),p.colors.background.slice(i,i+2)));colors.set(hex,`rgb(${parts})`);}return colors.get(hex);}
 function paint(frame){
  if(frame.full||frame.cols!==cols||frame.rows!==rows){cols=frame.cols;rows=frame.rows;grid=Array.from({length:cols*rows},()=>[' ',1,p.colors.foreground,p.colors.background,0]);canvas.width=cols*cw;canvas.height=rows*ch;canvas.style.width=`${cols*cw/scale}px`;canvas.style.height=`${rows*ch/scale}px`;}
  for(const [x,y,s,w,fg,bg,flags] of frame.cells)grid[y*cols+x]=[s,w,fg,bg,flags];
  // Native backgrounds first, then glyphs: italic/symbol overhang never erases a neighbour.
  for(let y=0;y<rows;y++)for(let x=0;x<cols;){const cell=grid[y*cols+x];let end=x+cell[1];while(end<cols&&grid[y*cols+end][3]===cell[3])end+=grid[y*cols+end][1];ctx.fillStyle=background(cell[3]);ctx.fillRect(x*cw,y*ch,(end-x)*cw,ch);x=end;}
  ctx.textBaseline='alphabetic';let lastFont='',lastColor='';
  for(let y=0;y<rows;y++)for(let x=0;x<cols;){const [s,w,fg,,flags]=grid[y*cols+x];if(s.trim()){
   // Configured Heavy Italic falls back to Regular in the local Alacritty setup.
   const font=`${(flags&3)===3?'':flags&2?'italic ':flags&1?'bold ':''}${fontSize}px LazyReviewTerminal`;
   if(font!==lastFont){ctx.font=font;lastFont=font;}if(fg!==lastColor){ctx.fillStyle=fg;lastColor=fg;}
   ctx.fillText(s,x*cw,y*ch+baseline);
   if(flags&4)ctx.fillRect(x*cw,y*ch+ch-4,w*cw,2);
   if(flags&8)ctx.fillRect(x*cw,y*ch+ch/2,w*cw,2);
  }x+=w;}
  state=frame.state;let text='';for(let y=0;y<rows;y++){for(let x=0;x<cols;){const cell=grid[y*cols+x];text+=cell[0];x+=cell[1];}text+='\n';}$('transcript').textContent=text;
  if(frame.copy){copy=frame.copy;$('copy').hidden=false;void navigator.clipboard?.writeText(copy).catch(()=>{});}
 }
 async function request(body){const response=await fetch(api,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(body)});const frame=await response.json();if(!response.ok||frame.error)throw Error(frame.error||'Demo unavailable');return frame;}
 function fail(error){$('error').textContent=error.message;$('failure').hidden=false;$('loading').hidden=true;}
 function schedule(){if(!busy&&!flushTimer)flushTimer=setTimeout(()=>{flushTimer=null;void flush();},12);}
 function send(event){if(!session||closed)return;queue.push(event);if(queue.length>256)queue=queue.slice(-256);schedule();}
 async function flush(){
  if(busy||!session||!queue.length||closed)return;busy=true;const current=generation,id=session,events=queue.splice(0,64);
  try{const frame=await request({session:id,events});if(current===generation)paint(frame);}catch(error){if(current===generation){queue=[];fail(error);}}finally{busy=false;if(queue.length)schedule();}
 }
 function size(){return {cols:Math.max(40,Math.min(220,Math.floor(stage.clientWidth/(cw/scale)))),rows:Math.max(16,Math.min(70,Math.floor(stage.clientHeight/(ch/scale))))};}
 async function reset(){
  const previous=session,current=++generation;session=null;queue=[];copy='';$('copy').hidden=true;$('failure').hidden=true;$('loading').hidden=false;clearInterval(pollTimer);
  if(previous)void request({session:previous,action:'close'}).catch(()=>{});
  try{await Promise.all([document.fonts.load(`${fontSize}px LazyReviewTerminal`),document.fonts.load(`bold ${fontSize}px LazyReviewTerminal`),document.fonts.load(`italic ${fontSize}px LazyReviewTerminal`)]);const frame=await request({action:'create',...size()});if(current!==generation){void request({session:frame.session,action:'close'}).catch(()=>{});return;}session=frame.session;paint(frame);$('loading').hidden=true;input.focus({preventScroll:true});pollTimer=setInterval(()=>{if(!document.hidden&&!busy&&!queue.length)send({type:'poll'});},1000);}catch(error){if(current===generation)fail(error);}
 }
 function showKey(key){const labels={Enter:'↵',Escape:'esc',Tab:'⇥',ArrowDown:'↓',ArrowUp:'↑',' ':'space'};$('keycast').textContent=labels[key]||key;$('keycast').hidden=false;clearTimeout(keyTimer);keyTimer=setTimeout(()=>$('keycast').hidden=true,900);}
 function key(name,props={}){showKey(name);send({type:'key',key:name,...props});}
 input.addEventListener('keydown',event=>{
  if(event.isComposing||event.metaKey||['Shift','Control','Alt','Meta','Dead','Process','Unidentified'].includes(event.key))return;
  if(event.ctrlKey&&event.key===']'){event.preventDefault();$('help').focus();return;}
  if(event.ctrlKey&&['v','c'].includes(event.key.toLowerCase()))return;
  if(event.key.length!==1&&!['Enter','Escape','Tab','Backspace','Delete','ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Home','End','PageUp','PageDown'].includes(event.key))return;
  event.preventDefault();key(event.key,{ctrl:event.ctrlKey,alt:event.altKey,shift:event.shiftKey});
 });
 input.addEventListener('paste',event=>{event.preventDefault();send({type:'paste',text:event.clipboardData.getData('text/plain').slice(0,10000)});});
 input.addEventListener('input',()=>{if(input.value){send({type:'paste',text:input.value.slice(0,10000)});input.value='';}});
 let dragging=false,lastCell='';
 function mouse(event,action){const r=canvas.getBoundingClientRect(),x=Math.floor((event.clientX-r.left)/(cw/scale)),y=Math.floor((event.clientY-r.top)/(ch/scale));if(x<0||y<0||x>=cols||y>=rows)return;const cell=`${x},${y}`;if(action==='drag'&&cell===lastCell)return;lastCell=cell;send({type:'mouse',action,x,y,shift:event.shiftKey,ctrl:event.ctrlKey});}
 canvas.addEventListener('pointerdown',event=>{if(event.button!==0)return;event.preventDefault();input.focus({preventScroll:true});dragging=true;canvas.setPointerCapture(event.pointerId);mouse(event,'down');});
 canvas.addEventListener('pointermove',event=>{if(dragging)mouse(event,'drag');});
 canvas.addEventListener('pointerup',event=>{if(dragging)mouse(event,'up');dragging=false;input.focus({preventScroll:true});});canvas.addEventListener('pointercancel',()=>dragging=false);
 canvas.addEventListener('wheel',event=>{event.preventDefault();mouse(event,event.deltaY<0?'scrollUp':'scrollDown');},{passive:false});
 $('help').onclick=()=>{input.focus({preventScroll:true});key('?');};$('reset').onclick=reset;$('retry').onclick=reset;
 $('copy').onclick=async()=>{try{await navigator.clipboard.writeText(copy);$('copy').textContent='Copied';}catch{$('hint').textContent='Select text in the accessible terminal transcript to copy.';}};
 document.querySelectorAll('[data-key]').forEach(button=>button.onclick=()=>{input.focus({preventScroll:true});key(button.dataset.key);});$('type').onclick=()=>input.focus({preventScroll:true});
 new ResizeObserver(()=>{clearTimeout(resizeTimer);resizeTimer=setTimeout(()=>{const next=size();if(next.cols!==cols||next.rows!==rows)send({type:'resize',...next});},100);}).observe(stage);
 addEventListener('pagehide',()=>{closed=true;clearInterval(pollTimer);if(session)void fetch(api,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({action:'close',session}),keepalive:true}).catch(()=>{});});
 window.lazyreviewTui=Object.freeze({getState:()=>({...state,session,cols,rows,ready:!!session&&$('failure').hidden,selectionText:copy,queued:queue.length}),text:()=>$('transcript').textContent});
 void reset();
})();
