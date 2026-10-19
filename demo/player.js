(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const data = window.LR_TUI_DATA || JSON.parse($('demo-data').textContent);
  const frames = data.frames;
  if (!Array.isArray(frames) || !frames.length) {
    $('error').hidden = false;
    $('error').textContent = 'No recorded scenes were found. Rebuild this demo with the native terminal exporter.';
    return;
  }
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
  const starts = []; let total = 0;
  for (const frame of frames) { starts.push(total); total += frame.duration; }
  let time = 0, index = -1, playing = false, rate = 1, lastTick = null;
  let fullView = false, actualSize = false, frameRequest = null;
  const speeds = [1, 1.5, 2, .75];
  const camera = $('camera'), terminal = $('terminal'), viewport = $('viewport'), image = $('terminal-image');
  const raster = data.nativeImages;
  if (!raster) throw new Error('Rebuild native images before building the TUI player.');
  image.style.width = `${raster.width / raster.scale}px`;
  image.style.height = `${raster.height / raster.scale}px`;
  // Preload recorded cells, never a provider or GitHub endpoint.
  for (const frame of frames) { const preload = new Image(); preload.src = frame.image; }

  const chapters = [];
  frames.forEach((frame, i) => {
    const title = frame.chapter || 'The workspace';
    if (!chapters.some(c => c.title === title)) chapters.push({ title, index: i });
  });
  chapters.forEach(chapter => {
    const button = document.createElement('button');
    button.type = 'button'; button.className = 'chapter'; button.textContent = chapter.title;
    button.addEventListener('click', () => seek(starts[chapter.index], true));
    $('chapters').append(button); chapter.button = button;
  });
  function clock(ms) { const seconds = Math.floor(ms / 1000); return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`; }
  function findFrame(position) {
    for (let i = frames.length - 1; i >= 0; i--) if (position >= starts[i]) return i;
    return 0;
  }
  function setCamera(instant = false) {
    const frame = frames[index]; if (!frame) return;
    const width = viewport.clientWidth, height = viewport.clientHeight;
    if (actualSize) {
      camera.style.transition = 'none'; camera.style.transform = 'none';
      const focus = frame.focus || [0, 0, data.cols, data.rows];
      viewport.scrollTo({left: Math.max(0, (focus[0] + focus[2]/2) * raster.width/raster.scale/data.cols - width/2), top: Math.max(0, (focus[1] + focus[3]/2) * raster.height/raster.scale/data.rows - height/2), behavior:'instant'});
      return;
    }
    const rawWidth = raster.width / raster.scale + 6, rawHeight = raster.height / raster.scale + 20;
    const line = raster.height / raster.scale / data.rows;
    if (!width || !height || !rawWidth) return;
    const fit = Math.min(width / rawWidth, height / rawHeight);
    const focus = frame.focus && !fullView && !reducedMotion.matches ? frame.focus : null;
    let scale = fit, x = (width - rawWidth * fit) / 2, y = (height - rawHeight * fit) / 2;
    if (focus) {
      const unit = raster.width / raster.scale / data.cols;
      const [fx, fy, fw, fh] = focus;
      scale = Math.min(width / ((fw + 6) * unit), height / ((fh + 3) * line), fit * 2.6);
      scale = Math.max(scale, fit);
      x = width / 2 - (fx + fw / 2) * unit * scale;
      y = height / 2 - (fy + fh / 2) * line * scale;
      x = Math.max(width - rawWidth * scale, Math.min(0, x));
      y = rawHeight * scale <= height ? (height - rawHeight * scale) / 2 : Math.max(height - rawHeight * scale, Math.min(0, y));
      if (fy <= 2) y = 0;
    }
    camera.style.transition = instant || reducedMotion.matches ? 'none' : 'transform 720ms var(--ease-in-out)';
    camera.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
    $('focus-label').hidden = !focus || !frame.focusLabel;
    $('focus-label').textContent = frame.focusLabel || frame.chapter || '';
  }
  function showFrame(next, instant) {
    index = next; const frame = frames[index];
    terminal.innerHTML = frame.html.replace(/^\s*<pre(?:\s[^>]*)?>/i, '').replace(/<\/pre>\s*$/i, '');
    image.src = frame.image;
    $('terminal-text').textContent = terminal.textContent;
    const card = frame.kind === 'card';
    document.querySelector('.presentation').classList.toggle('is-card', card);
    $('title-card').hidden = !card;
    $('card-title').textContent = frame.title;
    $('card-caption').textContent = frame.caption;
    $('card-chapter').textContent = frame.chapter;
    $('scene-title').textContent = frame.title;
    $('scene-caption').textContent = frame.caption;
    $('chapter-label').textContent = frame.chapter || 'The workspace';
    const chapterIndex = chapters.findIndex(c => c.title === (frame.chapter || 'The workspace'));
    $('chapter-number').textContent = String(chapterIndex + 1).padStart(2, '0');
    chapters.forEach((c, i) => c.button.setAttribute('aria-current', String(i === chapterIndex)));
    $('scene-count').textContent = `${String(index + 1).padStart(2, '0')} / ${String(frames.length).padStart(2, '0')}`;
    $('key-sequence').replaceChildren();


    $('previous').disabled = index === 0;
    $('next').disabled = index === frames.length - 1;
    setCamera(instant);
    // Cards and camera changes explain the workflow. Direct navigation stays immediate.
    if (!instant && !reducedMotion.matches && card) {
      $('title-card').getAnimations().forEach(a => a.cancel());
      $('title-card').animate([{ opacity: 0, transform: 'translateY(10px)' }, { opacity: 1, transform: 'translateY(0)' }], { duration: 500, easing: 'cubic-bezier(0.23, 1, 0.32, 1)' });
    }
    // Only autoplay gets explanatory motion. User navigation always updates immediately.
    if (!instant && !reducedMotion.matches) {
      document.querySelector('.scene-heading').getAnimations().forEach(a => a.cancel());
      document.querySelector('.scene-heading').animate([{ opacity: .35, transform: 'translateY(5px)' }, { opacity: 1, transform: 'translateY(0)' }], { duration: 250, easing: 'cubic-bezier(0.23, 1, 0.32, 1)' });
    }
  }
  function showKeys(instant) {
    const frame = frames[index], cast = frame.keycast;
    const within = time - starts[index];
    const recent = cast.events.filter(event => event.at <= within);
    const visible = recent.length > 0 && within < cast.end - cast.fade;
    const overlay = $('key-overlay'), sequence = $('key-sequence');
    overlay.style.transitionDuration = instant ? '0ms' : '';
    overlay.classList.toggle('visible', visible);
    // Future keys do not exist in the DOM. Scrubbing backwards also removes them.
    while (sequence.children.length > recent.length) sequence.lastChild.remove();
    for (let i = sequence.children.length; i < recent.length; i++) {
      const event = recent[i], cap = document.createElement(event.kind === 'key' ? 'kbd' : 'span');
      cap.textContent = event.label;
      cap.setAttribute('aria-label', event.key);
      cap.className = event.kind === 'key' ? 'key-pill' : 'mouse-label';
      sequence.append(cap);
      if (!instant && !reducedMotion.matches) cap.animate(
        [{ opacity: .35, transform: 'translateY(3px)' }, { opacity: 1, transform: 'translateY(0)' }],
        { duration: 160, easing: 'cubic-bezier(0.23, 1, 0.32, 1)' });
    }
    [...sequence.children].forEach((cap, i) => cap.classList.toggle('latest', within - recent[i].at < 200));
  }
  function update(instant = false) {
    const next = findFrame(time);
    if (next !== index) showFrame(next, instant);
    $('elapsed').textContent = clock(time);
    $('scrubber').value = Math.round(time / total * 1000);
    $('scrubber').style.background = `linear-gradient(to right, #9ebfe9 0%, #9ebfe9 ${time / total * 100}%, #273142 ${time / total * 100}%, #273142 100%)`;
    $('scrubber').setAttribute('aria-valuetext', `${clock(time)} of ${clock(total)}. ${frames[index].title}`);
    showKeys(instant);
    $('replay-overlay').hidden = time < total;
  }
  function setPlaying(value) {
    playing = value; lastTick = null;
    $('play-label').textContent = playing ? 'Pause' : time >= total ? 'Replay' : 'Play';
    $('play').setAttribute('aria-label', playing ? 'Pause demo' : time >= total ? 'Replay demo' : 'Play demo');
    $('play-icon').innerHTML = playing ? '<path d="M5 4h3.5v12H5zm6.5 0H15v12h-3.5z"/>' : '<path d="M6 3.5 16 10 6 16.5Z"/>';
    if (frameRequest !== null) cancelAnimationFrame(frameRequest);
    frameRequest = playing ? requestAnimationFrame(tick) : null;
  }
  function tick(now) {
    if (!playing) return;
    if (lastTick !== null) time = Math.min(total, time + Math.min(now - lastTick, 250) * rate);
    lastTick = now; update(false);
    if (time >= total) setPlaying(false);
    else frameRequest = requestAnimationFrame(tick);
  }
  function seek(position, instant = true) {
    time = Math.max(0, Math.min(total, position)); lastTick = null; update(instant); setCamera(instant);
    if (time >= total) setPlaying(false);
  }
  function toggle() { if (time >= total) seek(0); setPlaying(!playing); }
  function go(delta) { seek(starts[Math.max(0, Math.min(frames.length - 1, index + delta))], true); }
  $('play').addEventListener('click', toggle);
  $('previous').addEventListener('click', () => go(-1));
  $('next').addEventListener('click', () => go(1));
  $('replay-overlay').addEventListener('click', () => { seek(0); setPlaying(true); });
  $('home').addEventListener('click', event => { event.preventDefault(); seek(0); setPlaying(false); });
  $('scrubber').addEventListener('input', () => seek(Number($('scrubber').value) / 1000 * total));
  $('speed').addEventListener('click', () => { rate = speeds[(speeds.indexOf(rate) + 1) % speeds.length]; $('speed').textContent = `${rate}×`; $('speed').setAttribute('aria-label', `Playback speed ${rate} times`); });
  $('actual-size').addEventListener('click', () => {
    actualSize = !actualSize;
    $('actual-size').setAttribute('aria-pressed', String(actualSize));
    viewport.classList.toggle('actual-size', actualSize);
    viewport.scrollTo(0, 0); setCamera(true);
  });
  $('fit-camera').addEventListener('click', () => { fullView = !fullView; $('fit-camera').setAttribute('aria-pressed', String(fullView)); setCamera(true); });
  document.addEventListener('keydown', event => {
    if (event.altKey || event.ctrlKey || event.metaKey || event.target.matches('input, textarea, select, button, a')) return;
    if (event.code === 'Space') { event.preventDefault(); toggle(); }
    else if (event.key === 'ArrowRight') { event.preventDefault(); go(1); }
    else if (event.key === 'ArrowLeft') { event.preventDefault(); go(-1); }
    else if (event.key === 'Home') { event.preventDefault(); seek(0); }
  });
  document.addEventListener('visibilitychange', () => { if (document.hidden) setPlaying(false); });
  new ResizeObserver(() => setCamera(true)).observe(viewport);
  reducedMotion.addEventListener('change', () => { if (reducedMotion.matches) setPlaying(false); setCamera(true); });
  $('duration').textContent = clock(total);

  update(true);
  // A direct file open stays still until Play. This makes the first frame readable,
  // preserves reduced-motion preferences, and avoids unsolicited autoplay.
  if (new URLSearchParams(location.search).get('play') === '1' && !reducedMotion.matches) setPlaying(true);
  // Small read-only test surface; no network, backend, Claude, or GitHub operations.
  window.reviewDemo = Object.freeze({ seek: seconds => seek(seconds * 1000), pause: () => setPlaying(false), play: () => { if (time >= total) seek(0); setPlaying(true); }, getState: () => ({ index, time, total, playing, rate, actualSize, fullView, imageReady: image.complete && image.naturalWidth === raster.width, frame: frames[index].id }) });
})();
