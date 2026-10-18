"""Full sample review through the shipped executable in an isolated terminal."""
import errno,fcntl,os,pathlib,select,struct,subprocess,termios,time
from vt_screen import Screen
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/release/lazyreview'))
master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',45,150,0,0))
proc=subprocess.Popen([binary,'--demo'],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'TERM':'xterm-256color'},start_new_session=True)
os.close(slave);screen=Screen();seen=bytearray()
def read(seconds=.18):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        ready,_,_=select.select([master],[],[],min(.03,max(0,deadline-time.monotonic())))
        if ready:
            try:data=os.read(master,65536)
            except OSError as e:
                if e.errno==errno.EIO:break
                raise
            if not data:break
            seen.extend(data);screen.feed(data)
def send(keys,seconds=.18):os.write(master,keys.encode());read(seconds)
def has(text):assert text in screen.text(), 'Expected rendered text: '+text
def paste(text):send('\x1b[200~'+text+'\x1b[201~')
try:
    read(.6);has('sample');assert 'LazyReview' not in screen.text();assert 'AI companion' not in screen.text();has('+41');has('−10')
    send('2');has('Tests');send('1');send('\t');send('v');send(' ');send('j');send('v');send(' ')
    send('c');has('Comment · draft');has('linked ranges')
    # Compact modal leaves the surrounding tree and diff visible.
    has('Files ·');has('Changes · inline')
    paste('**Compare both ranges**\n- [ ] Cover the edge case');has('Comment · draft')
    send('\r',.4);has('Draft saved');assert 'Comment · draft' not in screen.text()
    send('c');send('@');has('Enter/Tab insert');send('\t');send('please check #42');has('#42');send('\t');send('too');send('\r',.4);has('Draft saved')
    send('r');has('Your review');has('Drafts 2');assert 'Compare both ranges' not in screen.text();send('ljl');has('Compare both ranges');assert '**Compare both ranges**' not in screen.text();has('Related code:');send('h');assert 'Related code:' not in screen.text()
    send('S');has('Review changes');paste('Reviewed the code and its references.');send('\r');has('Publish review');has('2 draft comments')
    send('\r',.4);has('Review changes (0)');has('Posted')
    send('b');send('G');send('i');has('Reply');paste('Reply with `code`.');send('\r',.4);has('Reply with code.')
    send('R');has('Thread state');send('\r',.4);has('Resolved');send('R');send('\r',.4)
    send('p');send('i');has('PR comment');paste('General **comment**');send('\r',.4);has('General comment')
    send('r');has('Your review');send('P');send('G');send('l');send('G');send('l');has('General comment')
    send('g?');send('f');send('range');has('Pin/unpin selected range');send('\x1b');send('\x1b')
    send('q');proc.wait(timeout=3);assert proc.returncode==0
    for code in [b'\x1b[?1049l',b'\x1b[?1006l',b'\x1b[?2004l']:assert code in seen
    assert b'panicked' not in seen
    print('PASS terminal review: multiple ranges, compact composer, multiline paste, @/# completion, Enter drafts, Markdown summary, review publication, replies, resolution, general comments, terminal restoration')
finally:
    if proc.poll() is None:proc.kill();proc.wait()
    os.close(master)
