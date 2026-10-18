"""Offline floating repository/PR search through the real terminal event loop."""
import errno,fcntl,os,pathlib,select,shutil,struct,subprocess,tempfile,termios,time
from vt_screen import Screen
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/debug/lazyreview'))
with tempfile.TemporaryDirectory(prefix='rw-picker-search-') as td:
    td=pathlib.Path(td);stub=td/'bin';stub.mkdir();node=shutil.which('node');wrapper=stub/'node'
    wrapper.write_text('#!/usr/bin/env python3\nimport os,sys\nargs=sys.argv[1:]\nif any(x.endswith("/bridge.mjs") for x in args):args=['+repr(str(root/'tests/picker-search-backend.mjs'))+']\nos.execv('+repr(node)+',['+repr(node)+',*args])\n');wrapper.chmod(0o700)
    env={**os.environ,'PATH':str(stub)+os.pathsep+os.environ['PATH'],'TERM':'xterm-256color','XDG_CONFIG_HOME':str(td/'config'),'XDG_STATE_HOME':str(td/'state')}
    master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',42,145,0,0))
    proc=subprocess.Popen([binary],stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True);os.close(slave);screen=Screen(145,42)
    def read(seconds=.12):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            if select.select([master],[],[],.02)[0]:
                try:data=os.read(master,65536)
                except OSError as e:
                    if e.errno==errno.EIO:return
                    raise
                screen.feed(data)
    def send(text):os.write(master,text.encode());read()
    def wait(text,seconds=4):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            read(.05)
            if text in screen.text():return
        raise AssertionError('Missing '+repr(text)+'\n'+screen.text())
    try:
        wait('acme/current');send('/');wait('archive/library')
        send('arclib');wait('1 repositories');send('\r');wait('Your PRs');wait('#42');wait('closed')
        send('14 fx');wait('1 matches');assert '#42' not in screen.text();send('\r');wait('Changes · inline')
        send('\x10');wait('Repositories')
        # The visible Search control opens the same floating workflow.
        rows=screen.text().splitlines();y=next(i for i,r in enumerate(rows) if 'Search /' in r);x=rows[y].index('Search /')+1
        send(f'\x1b[<0;{x};{y+1}M\x1b[<0;{x};{y+1}m');wait('archive/library')
        send('\x12');send('\x1b');send('\x1b');wait('Changes · inline');read(1);assert 'Changes · inline' in screen.text(), 'Late search result replaced the active review'
        send('\x10');wait('Repositories');send('/');wait('archive/library');send('\x1b[200~someone/project\x1b[201~');wait('Browse this repository');send('\r');wait('All PRs');wait('#1234')
        send('234 ext');wait('1 matches');send('\r');wait('Changes · inline');send('q');proc.wait(timeout=5);assert proc.returncode==0
        print('PASS floating search: fuzzy repositories, closed/merged authored PRs, number/title matching, clickable Search, Escape, pasted external repo; offline')
    finally:
        if proc.poll() is None:proc.terminate();proc.wait(timeout=5)
        os.close(master)
