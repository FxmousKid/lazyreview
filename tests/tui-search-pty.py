"""Pane-local search, split-side selection and review toolbar in a real PTY."""
import errno,fcntl,os,pathlib,select,struct,subprocess,termios,time
from vt_screen import Screen
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/release/lazyreview'))
master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',45,150,0,0))
proc=subprocess.Popen([binary,'--demo'],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'TERM':'xterm-256color'},start_new_session=True)
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
    read(.6);has('Review changes (0)');assert 'pinned ranges' not in screen.text()
    send('/');has('Filter files');send('command-menu.tsx\r');has('Files · 1/18')
    send('\t');send('/');send('useState');has('/useState▏');has('Both');assert 'Filter files' not in screen.text()
    send('\r');send('n');has('2/3');send('N');has('1/3')
    send('s');has('side by side');send('/');send('useState');send('\t');has('Original');send('\t');has('Modified');send('\r');send('n');has('2/2')
    send('/');send('no such phrase');has('0/0');send('\x1b');has('/useState');has('Modified');send('\x1b');assert '/useState' not in screen.text()
    # Locate the actual rendered button before clicking it.
    for y,row in enumerate(screen.rows):
        line=''.join(row)
        if 'Review changes (0)' in line:
            x=line.index('Review changes (0)')+2;break
    send(f'\x1b[<0;{x+1};{y+1}M');send(f'\x1b[<0;{x+1};{y+1}m');has('Request changes');has('Approve');send('\x1b')
    send('q');proc.wait(timeout=3);assert proc.returncode==0
    assert b'\x1b[?1049l' in seen and b'panicked' not in seen
    print('PASS terminal: file/diff search isolation, incremental highlights, n/N, split scopes, Escape restoration, mouse review action')
finally:
    if proc.poll() is None:proc.kill();proc.wait()
    os.close(master)
