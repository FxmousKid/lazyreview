(() => {
  'use strict';
  const $ = (s, root = document) => root.querySelector(s);
  const $$ = (s, root = document) => [...root.querySelectorAll(s)];
  const stage = $('#playground'), sim = $('.sim'), dialog = $('#demo-dialog');
  const initialChat = $('#chat').innerHTML;
  const data = window.LR_SAMPLE;
  const escape = s => String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  let state, noticeTimer, prefixTimer, prefix = '', returnFocus;
  function notify(message) {
    $('#notice').textContent = message; $('#notice').classList.add('visible');
    clearTimeout(noticeTimer); noticeTimer = setTimeout(() => $('#notice').classList.remove('visible'), 3200);
  }
  function open(title, html) {
    returnFocus = document.activeElement;
    $('#dialog-title').textContent = title; $('#dialog-body').innerHTML = html;
    dialog.showModal();
  }
  function close() { dialog.close(); returnFocus?.focus({preventScroll:true}); }
  $('#dialog-close').onclick = close;
  dialog.addEventListener('click', e => {if (e.target === dialog) {const r = dialog.getBoundingClientRect(); if(e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) close();}});
  $('#copy-install').onclick = async () => {
    const code = $('.install-command code');
    try {await navigator.clipboard.writeText(code.textContent); notify('Commands copied');}
    catch {const r = document.createRange(); r.selectNodeContents(code); const sel = getSelection(); sel.removeAllRanges(); sel.addRange(r); notify('Commands selected. Copy with your keyboard.');}
  };
  const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  if ('IntersectionObserver' in window && !reduced) {
    const observer = new IntersectionObserver(entries => entries.forEach(e => {if(e.isIntersecting) {e.target.classList.add('is-visible'); observer.unobserve(e.target);}}), {threshold:.08});
    $$('[data-reveal]').forEach((el, i) => {el.style.transitionDelay = `${Math.min(i % 3, 2) * 65}ms`; observer.observe(el);});
    document.documentElement.classList.add('motion-ready');
  }
  if (!data?.files?.length) return;
  const files = data.files;
  function reset() {
    state = {file:0, category:'All', query:'', pane:'changes', viewed:new Set(), split:false, anchor:null, end:null, cursor:1, visual:false, notes:[], drafts:[]};
    $('#chat').innerHTML = initialChat; $('#chat-input').value = ''; $('#file-search').value = ''; render();
  }
  function current() {return files[state.file];}
  function matching() {return files.map((f,i) => ({...f, index:i})).filter(f => (state.category === 'All' || f.category === state.category) && fuzzy(f.filename, state.query));}
  function fuzzy(text, query) {let n=0; for(const c of text.toLowerCase()) if(c===query.toLowerCase()[n]) n++; return n===query.length;}
  function setPane(pane, focus = true) {
    state.pane = pane; sim.dataset.mobilePane = pane;
    $$('[data-pane]').forEach(b => {b.classList.toggle('active',b.dataset.pane === pane); b.setAttribute('aria-pressed',b.dataset.pane === pane);});
    if(focus) (pane === 'companion' ? $('#chat-input') : pane === 'files' ? $('#file-list button') || $('#file-search') : $('#diff')).focus({preventScroll:true});
  }
  function setFile(index) {state.file = index; state.anchor = state.end = null; state.cursor = Math.max(0,current().lines.findIndex(l => l.kind!=='hunk')); state.visual=false; render();}
  function renderFiles() {
    const shown=matching(); $('#file-count').textContent=`${shown.length} / ${files.length}`;
    let folder='', html='';
    for(const f of shown) {
      const path = f.filename.split('/'), name=path.pop(), parent=path.join('/');
      if(folder!==parent) {folder=parent; html+=`<div class="tree-folder" aria-hidden="true">▾ ${escape(parent)}/</div>`;}
      const notes=state.notes.filter(n=>n.path===f.filename).length;
      // taste-skill-ignore: native viewed-state checkmark, inside the data-ui file tree.
      html+=`<button class="file-entry ${state.file===f.index?'active':''}" role="treeitem" aria-selected="${state.file===f.index}" data-file="${f.index}" title="${escape(f.filename)}"><span class="file-status">${state.viewed.has(f.index)?'✓':'M'}</span>${notes?`<span class="note-badge" aria-label="${notes} private notes">${notes}</span>`:''}<span class="filename">${escape(name)}</span><span class="counts"><span class="plus">+${f.additions}</span><span class="minus">−${f.deletions}</span></span></button>`; // taste-skill-ignore: native viewed-state glyph inside data-ui.
    }
    $('#file-list').innerHTML = html || '<p class="empty-files">No matching files</p>';
    $$('[data-file]').forEach(b => b.onclick=()=>{setFile(+b.dataset.file);setPane('changes');});
    $$('[data-filter]').forEach(b=>{b.classList.toggle('active',b.dataset.filter===state.category);b.setAttribute('aria-pressed',b.dataset.filter===state.category);});
  }
  function highlight(text) {
    // A tiny demo tokenizer. The native application uses bat/syntect.
    const regex=/(\/\/.*$|'(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*"|\b(?:const|let|return|export|function|if|else|import|from|type|interface|true|false|null|useEffect|useState)\b)/g;
    let out='', last=0;
    for(const m of text.matchAll(regex)) {out+=escape(text.slice(last,m.index));const cls=m[0].startsWith('//')?'comment':/^['"]/.test(m[0])?'string':'keyword';out+=`<span class="tok-${cls}">${escape(m[0])}</span>`;last=m.index+m[0].length;}
    return out+escape(text.slice(last));
  }
  function selectedIndices() {
    if(state.anchor===null) return [];
    const low=Math.min(state.anchor,state.end),high=Math.max(state.anchor,state.end);
    const anchor=current().lines[state.anchor];
    return current().lines.map((line,i)=>({line,i})).filter(({line,i})=>i>=low&&i<=high&&line.kind!=='hunk'&&line.hunk===anchor.hunk&&(anchor.kind==='delete'?line.old!==undefined:line.new!==undefined)).map(({i})=>i);
  }
  function row(line,index,side) {
    if(!line) return '<div class="diff-empty" aria-hidden="true"></div>';
    if(line.kind==='hunk') return `<div class="diff-line hunk">${escape(line.text)}</div>`;
    const selected=selectedIndices().includes(index), num=side==='old'?line.old:side==='new'?line.new:null;
    return `<button type="button" class="diff-line ${line.kind} ${selected?'selected':''} ${state.cursor===index?'cursor':''}" data-line="${index}" aria-pressed="${selected}" title="Click to select; Shift-click to extend"><span class="gutter">${side?num??'':line.old??''}</span>${side?'':`<span class="gutter">${line.new??''}</span>`}<code>${line.kind==='add'?'+':line.kind==='delete'?'−':' '} ${highlight(line.text)}</code></button>`;
  }
  function renderDiff() {
    const file=current(), lines=file.lines, selected=selectedIndices();
    $('#current-file').textContent=file.filename; $('#current-file').title=file.filename;
    const diff=$('#diff'), scroll=diff.scrollTop;
    if(state.split) {
      // Pair contiguous deleted and added blocks, padding the shorter side.
      const pairs=[]; let i=0;
      while(i<lines.length) {
        const l=lines[i];
        if(l.kind==='hunk'||l.kind==='context'){pairs.push([i,i]);i++;continue;}
        const left=[],right=[];
        while(i<lines.length&&['add','delete'].includes(lines[i].kind)){(lines[i].kind==='delete'?left:right).push(i++);}
        for(let p=0;p<Math.max(left.length,right.length);p++)pairs.push([left[p],right[p]]);
      }
      diff.innerHTML=`<div class="split-columns">${['old','new'].map((side,column)=>`<div class="split-half"><div class="split-title">${column?'Modified':'Original'}</div>${pairs.map(pair=>row(lines[pair[column]],pair[column],side)).join('')}</div>`).join('')}</div>`;
    } else diff.innerHTML=lines.map((l,i)=>row(l,i)).join('');
    diff.scrollTop=scroll;
    $$('[data-line]',diff).forEach(b=>b.onclick=e=>{const i=+b.dataset.line; if(!e.shiftKey||state.anchor===null||lines[state.anchor].hunk!==lines[i].hunk)state.anchor=i;state.end=state.cursor=i;state.visual=false;setPane('changes',false);renderDiff();});
    $('#selection-label').textContent=selected.length?`${selected.length} line${selected.length===1?'':'s'} selected`:'Click a line · Shift-click for a range';
    $('#note-toggle').disabled=!selected.length;
    $('#layout-toggle').textContent=state.split?'Inline':'Split';
    $('#viewed-toggle').setAttribute('aria-pressed',state.viewed.has(state.file));
    // taste-skill-ignore: viewed-state checkmark mirrors the real terminal control.
    $('#viewed-toggle').textContent=state.viewed.has(state.file)?'✓ Viewed':'Viewed'; // taste-skill-ignore: native viewed-state glyph inside data-ui.
  }
  function render() {renderFiles();renderDiff();setPane(state.pane,false);$('#viewed-count').textContent=`${state.viewed.size} / ${files.length} viewed`;$('#context-count').textContent=state.notes.length;$('#draft-count').textContent=state.drafts.length;}
  function selectedContext() {
    const indices=selectedIndices(); if(!indices.length)return null;
    const file=current(), first=file.lines[indices[0]],last=file.lines[indices.at(-1)], side=first.kind==='delete'?'LEFT':'RIGHT';
    return {path:file.filename, start:side==='LEFT'?first.old:first.new, end:side==='LEFT'?last.old:last.new, side, code:indices.map(i=>file.lines[i].text).join('\n')};
  }
  function compose(kind='note') {
    const context=selectedContext();if(!context){notify('Select a code line first');setPane('changes');return;}
    const privateNote=kind==='note';
    open(privateNote?'Private AI note':'Sample GitHub comment',`<p class="small">${escape(context.path)} · ${context.start}–${context.end} · ${context.side.toLowerCase()}</p><pre>${escape(context.code)}</pre><form id="note-form"><label for="note-body">${privateNote?'What should Claude look at?':'Draft a comment in this local sample'}</label><textarea id="note-body" required placeholder="${privateNote?'How does this interact with the other file?':'What would you like to say?'}"></textarea><button class="button primary" type="submit">${privateNote?'Queue private note':'Save sample draft'}</button></form>`);
    $('#note-form').onsubmit=e=>{e.preventDefault();const body=$('#note-body').value.trim();if(!body)return;(privateNote?state.notes:state.drafts).push({...context,body});close();render();notify(privateNote?'Queued for your next question. Add another file, or ask Claude.':'Draft saved in this browser demo');};
    $('#note-body').focus();
  }
  function showContext() {
    open('Queued AI context',state.notes.length?`<p class="small">Private notes travel with the next question. Nothing is sent by this browser sample.</p>${state.notes.map((n,i)=>`<div class="context-note"><strong>${escape(n.path)}</strong><span> · ${n.start}–${n.end}</span><button class="remove-note" data-remove="${i}">Remove</button><p>${escape(n.body)}</p><pre>${escape(n.code)}</pre></div>`).join('')}`:'<p>Select code, then choose “Ask about this” to queue a private question. You can attach code from multiple files.</p>');
    $$('[data-remove]').forEach(b=>b.onclick=()=>{state.notes.splice(+b.dataset.remove,1);close();render();showContext();});
  }
  function showReview() {
    open('Your sample review',`<p class="small">Local demo only. These comments do not go to GitHub.</p>${state.drafts.length?state.drafts.map(d=>`<details><summary>${escape(d.path)} · ${d.start}–${d.end}</summary><pre>${escape(d.code)}</pre><p>${escape(d.body)}</p></details>`).join(''):'<p>No draft comments yet. Select code and press c to add one.</p>'}<div class="outcomes"><button class="button primary" data-outcome="Comment">Comment</button><button class="button primary" data-outcome="Approve">Approve</button><button class="button primary" data-outcome="Request changes">Request changes</button></div>`);
    $$('[data-outcome]').forEach(b=>b.onclick=()=>{const outcome=b.dataset.outcome;open('Review preview',`<p><strong>${escape(outcome)}</strong> · ${state.drafts.length} sample comment${state.drafts.length===1?'':'s'}</p><p>This is where the terminal app asks you to confirm publication. This browser demo makes no GitHub request.</p><button type="button" class="button primary" id="back-review">Back to drafts</button>`);$('#back-review').onclick=showReview;});
  }
  function ask(event) {
    event.preventDefault();const input=$('#chat-input'),question=input.value.trim();if(!question)return;
    const chat=$('#chat'),q=document.createElement('p');q.className='chat-question';q.textContent=question;chat.append(q);
    const answer=document.createElement('div');answer.className='answer';
    if(state.notes.length){const chip=document.createElement('span');chip.className='context-used';chip.textContent='Sample context · '+[...new Set(state.notes.map(n=>`${n.path}:${n.start}–${n.end}`))].join(' · ');answer.append(chip);}
    const p=document.createElement('p');
    p.textContent=state.notes.length?'These selections connect the keyboard handler to the menu state. Check that the active index resets when the filtered results change, and that Enter reads from those same filtered results.':'The PR connects three pieces: the command menu, its keyboard-navigation hook, and tests for filtering and selection. A useful question is what happens when the search changes while the last result is selected.';
    answer.append(p);const label=document.createElement('p');label.className='context-used';label.textContent='Scripted demo reply · the native app uses your AI CLI';answer.append(label);chat.append(answer);input.value='';state.notes=[];render();chat.scrollTop=chat.scrollHeight;
  }
  function move(delta) {
    if(state.pane==='files'){const shown=matching();if(!shown.length)return;const i=shown.findIndex(f=>f.index===state.file);setFile(shown[Math.max(0,Math.min(shown.length-1,i+delta))].index);}
    else if(state.pane==='changes'){const lines=current().lines;let i=Math.max(0,Math.min(lines.length-1,state.cursor+delta));if(lines[i].kind==='hunk')i=Math.max(0,Math.min(lines.length-1,i+delta));state.cursor=i;if(state.visual)state.end=i;renderDiff();$(`[data-line="${i}"]`)?.scrollIntoView({block:'nearest',inline:'nearest'});}
  }
  $('#reset-demo').onclick=()=>{reset();notify('Sample review reset');};
  $('#try-review').addEventListener('click',()=>{setPane('changes');stage.focus({preventScroll:true});});
  $$('[data-pane]').forEach(b=>b.onclick=()=>setPane(b.dataset.pane));
  $$('[data-filter]').forEach(b=>b.onclick=()=>{state.category=b.dataset.filter;const shown=matching();if(shown.length&&!shown.some(f=>f.index===state.file))setFile(shown[0].index);else renderFiles();setPane('files',false);});
  $('#file-search').oninput=e=>{state.query=e.target.value;renderFiles();};
  $('#layout-toggle').onclick=()=>{state.split=!state.split;renderDiff();};
  $('#viewed-toggle').onclick=()=>{state.viewed.has(state.file)?state.viewed.delete(state.file):state.viewed.add(state.file);render();};
  $('#note-toggle').onclick=()=>compose();$('#context-toggle').onclick=showContext;$('#review-toggle').onclick=showReview;$('#chat-form').onsubmit=ask;
  $('#chat-input').addEventListener('keydown',e=>{if(e.key==='Enter'&&!e.shiftKey&&!e.isComposing){e.preventDefault();ask(e);}});
  $$('.sim-panes>aside,.sim-panes>section').forEach(p=>p.addEventListener('focusin',()=>setPane(p.classList.contains('file-pane')?'files':p.classList.contains('changes-pane')?'changes':'companion',false)));
  $$('[data-action]').forEach(b=>b.onclick=()=>{if(b.dataset.action==='filter-code'){state.category='Code';state.query='';$('#file-search').value='';setFile(matching()[0].index);setPane('files');}else{state.category='All';state.query='';$('#file-search').value='';setFile(0);state.anchor=state.cursor;state.end=state.cursor+3;renderDiff();setPane('changes');notify('A range is selected. Choose “Ask about this”.');}});
  stage.addEventListener('keydown',e=>{
    const typing=/^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName)||e.target.isContentEditable;
    if(e.ctrlKey&&e.key===']'&&state.pane==='companion'){e.preventDefault();setPane('changes');return;}
    if(typing){if(e.key==='Escape'){e.preventDefault();stage.focus();}return;}
    if(e.ctrlKey||e.metaKey||e.altKey)return;
    const key=e.key;
    if(prefix==='g'){prefix='';clearTimeout(prefixTimer);if(key==='a'){e.preventDefault();setPane('companion');return;}if(key==='c'){e.preventDefault();showContext();return;}}
    if(key==='g'){prefix='g';clearTimeout(prefixTimer);prefixTimer=setTimeout(()=>prefix='',1000);e.preventDefault();return;}
    if(key==='j'||key==='k'){e.preventDefault();move(key==='j'?1:-1);}
    else if(key==='n'||key==='N'){e.preventDefault();const shown=matching();if(shown.length){const i=shown.findIndex(f=>f.index===state.file);setFile(shown[(i+(key==='n'?1:-1)+shown.length)%shown.length].index);}}
    else if(key==='h'||key==='l'){e.preventDefault();const panes=['files','changes','companion'];setPane(panes[Math.max(0,Math.min(2,panes.indexOf(state.pane)+(key==='l'?1:-1)))]);}
    else if(key==='/'){e.preventDefault();setPane('files',false);$('#file-search').focus();}
    else if(key==='v'){e.preventDefault();state.visual=!state.visual;state.anchor=state.end=state.cursor;renderDiff();}
    else if(key==='t'){e.preventDefault();compose();}
    else if(key==='c'){e.preventDefault();compose('comment');}
    else if(key==='s'){e.preventDefault();$('#layout-toggle').click();}
    else if(key==='m'){e.preventDefault();$('#viewed-toggle').click();}
    else if(key==='r'){e.preventDefault();showReview();}
    else if(key==='Escape'){state.anchor=state.end=null;state.visual=false;renderDiff();}
    else if(key==='?'){e.preventDefault();open('Sample keys','<p>j / k · move<br>n / N · next / previous file<br>h / l · change pane<br>/ · find a file<br>v then j / k · select lines<br>t · queue a private AI note<br>g c · inspect context<br>g a · ask Claude<br>Ctrl-] · return to changes<br>s · split / inline<br>m · viewed<br>c · sample GitHub comment<br>r · sample review</p><p class="small">Click and Shift-click also select code. The full terminal app has more keys; see its README.</p>');}
  });
  $('.fallback-workspace').hidden=true;sim.hidden=false;reset();
})();
