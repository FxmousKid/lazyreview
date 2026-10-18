"""Offline terminal controls smoke; every assertion uses the rendered screen."""
import errno, fcntl, os, pathlib, select, struct, subprocess, termios, time
from vt_screen import Screen
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/release/lazyreview'))
master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',45,150,0,0))
proc=subprocess.Popen([binary,'--demo','--theme','lazygit'],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'TERM':'xterm-256color'},start_new_session=True)
os.close(slave);screen=Screen();seen=bytearray()
def read(seconds=.2):
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
def send(keys):os.write(master,keys.encode());read()
def has(text):assert text in screen.text(), 'Expected rendered text: '+text+'\n'+screen.text()
def diff_x():return next(''.join(row).index(' Changes')-1 for row in screen.rows if ' Changes' in ''.join(row))
try:
    read(.7);has('Changes · inline');assert 'AI companion' not in screen.text()
    send('\t');send('s');has('side by side');send('s');has('Changes · inline')
    send('g?');has('Keyboard shortcuts');send('f');send('side by side');has('Toggle diff layout')
    for text in ['Requires Node.js','ANTHROPIC_API_KEY','terminal control characters','No network in sample mode']:assert text not in screen.text()
    send('\x1b');assert 'Keyboard shortcuts' not in screen.text();send('\\');has('Panes');send('\r');has('AI companion')
    # Restoring a pane focuses it; reopening the controls selects that pane.
    send('\\');send('\r');assert 'AI companion' not in screen.text()
    send('\t');before=diff_x();send('+');assert diff_x()>before
    send('=');before=diff_x()
    # Coordinates come from the observed divider, converted to SGR's one-based cells.
    send(f'\x1b[<0;{before};10M');send(f'\x1b[<32;{before+8};10M');send(f'\x1b[<0;{before+8};10m');assert diff_x()>before
    send('t');assert 'Files ·' not in screen.text();has('[+ Files]')
    rows=screen.text().splitlines();y=next(i for i,row in enumerate(rows) if '[+ Files]' in row);x=rows[y].index('[+ Files]')
    send(f'\x1b[<0;{x+1};{y+1}M');has('Files ·')
    assert 'AI companion' not in screen.text(), '= must preserve collapsed companion'
    send('ga');has('AI companion');send('Z');assert 'Files ·' not in screen.text();send('Z');has('Files ·')
    send('g!');send('f');send('copy');has('Keyboard shortcuts');send('\x1b');assert 'Keyboard shortcuts' not in screen.text()
    send('T');has('Theme: blue');send('T');has('Theme: custom');send('T');has('Theme: lazygit')
    send('\x1b[Z');send('\t')
    send('q');proc.wait(timeout=3);assert proc.returncode==0
    assert b'\x1b[?1049h' in seen and b'\x1b[?1049l' in seen
    assert b'\x1b[?1006h' in seen and b'\x1b[?1006l' in seen
    assert b'panicked' not in seen
    print('PASS terminal: Tab navigation, diff layouts, filterable help, folded panes, keyboard/mouse resize, zoom, all themes, terminal/mouse restoration')
finally:
    if proc.poll() is None:proc.kill();proc.wait()
    os.close(master)
