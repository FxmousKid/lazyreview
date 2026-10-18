"""Explicit read-only live chooser smoke. PR content stays in memory, never printed or saved."""
import errno, fcntl, os, pathlib, re, select, struct, subprocess, sys, termios, time
if '--live' not in sys.argv:
    raise SystemExit('Pass --live to allow reading your GitHub PR list and selected PR; no writes occur.')
from vt_screen import Screen
screen=Screen()
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/release/lazyreview'))
master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',45,150,0,0))
proc=subprocess.Popen([binary],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'TERM':'xterm-256color'},start_new_session=True)
os.close(slave)
seen=bytearray()
def read(seconds):
    chunk=bytearray();deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        ready,_,_=select.select([master],[],[],min(.05,max(0,deadline-time.monotonic())))
        if ready:
            try:data=os.read(master,65536)
            except OSError as e:
                if e.errno==errno.EIO:break
                raise
            if not data:break
            seen.extend(data);chunk.extend(data);screen.feed(data)
    return re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]','',chunk.decode('utf8',errors='replace'))
def wait_for(needle,label):
    captured='';deadline=time.monotonic()+60
    while time.monotonic()<deadline:
        read(.1)
        if needle in screen.text():return
        if proc.poll() is not None:raise AssertionError('Terminal exited during '+label)
    raise AssertionError('Timed out during '+label+'; no private terminal content is logged.')
def send(value):os.write(master,value.encode())
try:
    wait_for('Select a repository and Enter','authenticated repository load')
    print('PASS default executable lists repositories with authored open PRs',flush=True)
    send('/');wait_for('repositories · recent contributions first','complete authored history finder')
    send('\r');wait_for('newest first · open, merged and closed','authored PR history step')
    send('\x1b');read(.2)
    print('PASS floating search loads contribution repositories and all authored PR states',flush=True)
    send('\r');wait_for('newest updates first','repository PR list load')
    send('f');read(.15);send('NO_MATCH_CHOOSER_SMOKE\r');wait_for('No matches on this page','local filter')
    send('f');read(.1);send('\x15\r');read(.2)
    send('\r');wait_for('Changes · inline','selection to real PR review')
    print('PASS selecting a listed PR opens the real review screen',flush=True)
    send('\x10');wait_for('Repositories','return to chooser')
    send('4');read(.1);send('\x15primer/react\r');wait_for('Select a repository and Enter','repository search outside personal list')
    send('\r');wait_for('newest updates first','searched repository PRs')
    print('PASS repository search opens PR list independently of authored PRs',flush=True)
    send('\x1b');wait_for('Changes · inline','return to existing review')
    send('q');read(.3);proc.wait(timeout=3)
    assert proc.returncode==0
    assert b'\x1b[?1049h' in seen and b'\x1b[?1049l' in seen
    print('PASS filtering, Ctrl-p / Escape round trip, clean terminal restoration; no writes')
finally:
    if proc.poll() is None:proc.kill();proc.wait()
    os.close(master)
