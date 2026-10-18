"""Offline companion layouts, docked composer and description outline via real keys."""
import errno,fcntl,os,pathlib,select,struct,subprocess,termios,time
from vt_screen import Screen
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/release/lazyreview'))
master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',45,150,0,0))
proc=subprocess.Popen([binary,'--preview-ui','ai','--companion-style','paper','--theme','blue'],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'TERM':'xterm-256color'},start_new_session=True)
os.close(slave);screen=Screen();seen=bytearray()
def read(seconds=.2):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        ready,_,_=select.select([master],[],[],.02)
        if ready:
            try:data=os.read(master,65536)
            except OSError as e:
                if e.errno==errno.EIO:break
                raise
            if not data:break
            seen.extend(data);screen.feed(data)
def send(keys):os.write(master,keys.encode());read()
def has(value):assert value in screen.text(),'Expected '+value+'\n'+screen.text()
try:
    read(.8);has('AI companion');has('o Conversation');has('synthetic conversation')
    send('gv');has('Companion style');has('Ledger');has('Studio');send('k');send('\r');has('› You')
    send('gv');send('k');send('\r');has('You')
    send('gv');send('k');send('\r');has('o Conversation')
    send('F');has('F PR');send('F');has('F Focus')
    send('o');has('Conversation');has('Why reset');send('j');send('\r');has('What would you look at');send('gg')
    send('go');has('Artifacts');has('Saved outputs');send('\x1b')
    left=next(''.join(row).index(' AI companion')-1 for row in screen.rows if ' AI companion' in ''.join(row))
    before=[''.join(row)[:left] for row in screen.rows[2:40]]
    send('i');has('Ask Claude');has('Enter Send')
    assert before==[''.join(row)[:left] for row in screen.rows[2:40]], 'Chat composer covered the diff'
    send('Explain filtering');send('\x0e');send('and keyboard focus');has('keyboard focus')
    send('\x1b');send('D');has('Description');send('G');has('Rollout');has('100%')
    send('o');has('Description outline');send('gg') # outline ignores unrelated global actions
    send('kkkkkkkk');send('j');send('\r');has('What changed')
    send(']]');has('Implementation');send('[[');has('What changed')
    send('e');has('Ask Claude');has('Explain filtering');has('keyboard focus');send('\x1b')
    send('Z');has('AI companion');send('Z');send('q');proc.wait(timeout=3)
    assert proc.returncode==0 and b'panicked' not in seen and b'\x1b[?1049l' in seen
    print('PASS terminal: three styles, pane-local multiline composer, draft reopen, description outline, heading jumps, zoom, restoration')
finally:
    if proc.poll() is None:proc.kill();proc.wait()
    os.close(master)
