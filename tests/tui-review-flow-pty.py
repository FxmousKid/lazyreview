"""Real embedded sample backend: draft -> inspect -> outcome -> preview -> mouse publish.
The Node wrapper records operation names only and forwards the shipped bridge unchanged.
No GitHub API calls or writes are made.
"""
import errno, fcntl, json, os, pathlib, select, shutil, struct, subprocess, sys, tempfile, termios, time
from vt_screen import Screen
root = pathlib.Path(__file__).resolve().parent.parent
binary = os.environ.get('LAZYREVIEW_BINARY', str(root / 'tui/target/release/lazyreview'))
node = shutil.which('node')
assert node, 'Node is required for the embedded sample adapter'
with tempfile.TemporaryDirectory(prefix='lazyreview-submit-') as directory:
    tmp = pathlib.Path(directory)
    shim = tmp / 'bin'; shim.mkdir()
    log = tmp / 'operations.jsonl'
    wrapper = shim / 'node'
    wrapper.write_text('#!' + sys.executable + '\n' + '''import json,os,subprocess,sys
child=subprocess.Popen([NODE,*sys.argv[1:]],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=sys.stderr,text=True,bufsize=1)
try:
 for line in sys.stdin:
  try:
   value=json.loads(line)
   fd=os.open(os.environ['LAZYREVIEW_TEST_LOG'],os.O_WRONLY|os.O_APPEND|os.O_CREAT,0o600)
   os.write(fd,(json.dumps({'operation':value.get('operation'),'event':value.get('event')})+'\\n').encode());os.close(fd)
  except (ValueError,TypeError):pass
  child.stdin.write(line);child.stdin.flush()
  output=child.stdout.readline()
  if not output:break
  sys.stdout.write(output);sys.stdout.flush()
finally:
 child.stdin.close()
 try:child.wait(timeout=2)
 except subprocess.TimeoutExpired:child.kill();child.wait()
'''.replace('NODE', repr(node)))
    wrapper.chmod(0o755)
    env = {**os.environ, 'PATH':str(shim)+os.pathsep+os.environ['PATH'], 'TERM':'xterm-256color', 'XDG_CONFIG_HOME':str(tmp/'config'), 'XDG_STATE_HOME':str(tmp/'state'), 'LAZYREVIEW_TEST_LOG':str(log)}
    master, slave = os.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 45, 150, 0, 0))
    process = subprocess.Popen([binary, '--demo'], stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True)
    os.close(slave); screen=Screen(); seen=bytearray()
    def read(seconds=.15):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            if select.select([master],[],[],.02)[0]:
                try:data=os.read(master,65536)
                except OSError as error:
                    if error.errno==errno.EIO:break
                    raise
                if not data:break
                seen.extend(data);screen.feed(data)
    def send(value,seconds=.15):os.write(master,value.encode());read(seconds)
    def paste(value):send('\x1b[200~'+value+'\x1b[201~')
    def has(value):assert value in screen.text(), 'Missing '+repr(value)+'\n'+screen.text()
    def wait(value, timeout=5):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            read(.05)
            if value in screen.text():return
        has(value)
    def point(value,last=False):
        found=[(line.index(value)+1,y+1) for y,line in enumerate(screen.text().splitlines()) if value in line]
        assert found, 'Missing clickable '+repr(value)+'\n'+screen.text()
        return found[-1] if last else found[0]
    def mouse(code,x,y,up=False):send(f'\x1b[<{code};{x};{y}'+('m' if up else 'M'))
    def click(value,last=False):
        x,y=point(value,last);mouse(0,x,y);mouse(0,x,y,True)
    def submissions():return [v for v in map(json.loads,log.read_text().splitlines()) if v['operation']=='submit'] if log.exists() else []
    try:
        wait('sample')
        send('\t');send('c');paste('First boundary question\n\nFIRST DETAIL only when opened.');send('\r',.3);has('Draft saved')
        send('j');send('c');paste('Second focus question\n\nSECOND DETAIL only when opened.');send('\r',.3);has('Draft saved')
        send('r');has('Drafts 2');assert 'FIRST DETAIL' not in screen.text()
        send('l');has('First boundary');assert 'FIRST DETAIL' not in screen.text()
        send('jl');has('FIRST DETAIL');send('jl');has('SECOND DETAIL');assert 'FIRST DETAIL' not in screen.text()
        send('S');has('Review changes');click('○ Approve');has('● Approve')
        send('\t');has('● Request changes');send('\x1b[Z');has('● Approve');click('○ Request changes');has('● Request changes')
        paste('Please handle the empty-list boundary before merging.')
        send('\r');has('Publish review');has('2 draft comments');assert not submissions()
        send('\x1b');has('● Request changes');has('empty-list boundary')
        click('Continue');has('Publish review')
        x,y=point('Publish review',last=True)
        mouse(0,x,y);assert not submissions(), 'Mouse down must only arm publication'
        mouse(32,x+1,y);mouse(0,x+1,y,True);assert not submissions(), 'Dragging must never publish'
        mouse(0,x,y);mouse(0,x,y,True);read(.5)
        has('Review published in sample.');has('Drafts 0');has('Posted 3')
        mouse(0,x,y,True);read(.1)
        assert submissions()==[{'operation':'submit','event':'REQUEST_CHANGES'}],submissions()
        click('Review changes (0)');has('● Comment');assert 'empty-list boundary before merging' not in screen.text();send('\x1b')
        send('r');has('No draft comments.');click('Posted 3');has('src/')
        send('q');process.wait(timeout=3);assert process.returncode==0
        assert b'\x1b[?1049l' in seen and b'panicked' not in seen
        print('PASS real embedded sample: 2 drafts, single detail, keyboard/mouse outcomes, summary back, release-only Publish, exactly 1 submit, Posted history, no network')
    finally:
        if process.poll() is None:process.kill();process.wait()
        os.close(master)
