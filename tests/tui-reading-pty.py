"""PR reading navigation and real syntax colors in an isolated offline PTY."""
import errno,fcntl,os,pathlib,select,struct,subprocess,termios,time
from vt_screen import Screen
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/release/lazyreview'))
master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',45,150,0,0))
proc=subprocess.Popen([binary,'--demo'],stdin=slave,stdout=slave,stderr=slave,env={**{k:v for k,v in os.environ.items() if k!='NO_COLOR'},'TERM':'xterm-256color','COLORTERM':'truecolor'},start_new_session=True)
os.close(slave);screen=Screen();seen=bytearray()
def read(seconds=.15):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        ready,_,_=select.select([master],[],[],.02)
        if ready:
            try:data=os.read(master,65536)
            except OSError as e:
                if e.errno==errno.EIO:break
                raise
            if not data:break
            seen.extend(data);screen.feed(data)
def send(keys):os.write(master,keys.encode());read()
def has(text):assert text in screen.text(), 'Expected: '+text
try:
    read(.8);has('acme/atlas #128');has('Your review · 0')
    for label in ['LazyReview','LIVE GITHUB','NORMAL']:assert label not in screen.text()
    assert ''.join(screen.rows[-1]).startswith(' acme/atlas #128')
    send('D');has('Pull request');has('@')
    send('C');has('Pull request');has('Ready for review')
    send('gm');has('Pull request');has('36fb81a')
    send('D');send('o');has('PR timeline');send('\x1b')
    send('C');send('i');has('PR comment');send('\x1b')
    send('b');has('Review threads');has('Thread 1/')
    assert '╭ @' in screen.text() and '╰' in screen.text(), 'Thread comment has no card border'
    send('h');has('Pull request');send('b');has('Review threads')
    send('\\');send('\r');assert 'Review threads' not in screen.text()
    send('H');has('Syntax: Lunar')
    deadline=time.monotonic()+10
    while time.monotonic()<deadline:
        read(.1)
        if b'38;2;158;206;106' in seen:break
    assert b'38;2;158;206;106' in seen, 'Lunar string tokens were not rendered'
    send('H');has('Syntax: GitHub Dark')
    assert b'38;2;255;123;114' in seen, 'GitHub keyword tokens were not rendered'
    send('s');has('side by side')
    send('q');proc.wait(timeout=3);assert proc.returncode==0
    assert b'panicked' not in seen and b'\x1b[?1049l' in seen
    print('PASS terminal: compact controls, footer identity, unified timeline and thread cards, comment composer, both syntax palettes, split layout, restoration')
finally:
    if proc.poll() is None:proc.kill();proc.wait()
    os.close(master)
