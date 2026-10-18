"""Offline launcher → chooser → native typing regression. No model or GitHub calls."""
import errno,fcntl,json,os,pathlib,select,shutil,struct,subprocess,sys,tempfile,termios,time
from vt_screen import Screen
root=pathlib.Path(__file__).resolve().parent.parent
launcher=os.environ.get('LAZYREVIEW_BINARY',str(root/'lazyreview'))
setup='--setup' in sys.argv
with tempfile.TemporaryDirectory(prefix='rw-native-entry-') as td:
    td=pathlib.Path(td); config=td/'config';state=td/'state';stub=td/'bin';stub.mkdir();(config/'lazyreview').mkdir(parents=True)
    fixture=root/'tests/claude-terminal-fixture.py'
    (config/'lazyreview/config.yml').write_text('ai:\n  enabled: '+('true' if '--enabled' in sys.argv else 'false')+'\n  interface: '+('terminal' if setup else 'paper')+'\n  autoPrepare: false\n  command: '+json.dumps(str(fixture))+'\n')
    # Exercise the actual launcher and embedded bridges. Only the GitHub bridge
    # gets an offline transport; the Claude subprocess, hooks and session store
    # are the production implementation.
    node=shutil.which('node');wrapper=stub/'node'
    wrapper.write_text('#!/usr/bin/env python3\nimport os,sys\nargs=sys.argv[1:]\nif any(x.endswith("/bridge.mjs") for x in args):args=['+repr(str(root/'tests/native-entry-backend.mjs'))+']\nos.execv('+repr(node)+',['+repr(node)+',*args])\n');wrapper.chmod(0o700)
    clipboard=td/'clipboard';copy=stub/'pbcopy'
    copy.write_text('#!/usr/bin/env python3\nimport sys,pathlib\npathlib.Path('+repr(str(clipboard))+').write_text(sys.stdin.read())\n');copy.chmod(0o700)
    env={**os.environ,'PATH':str(stub)+os.pathsep+os.environ['PATH'],'TERM':'xterm-256color','XDG_CONFIG_HOME':str(config),'XDG_STATE_HOME':str(state)}
    master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',45,180,0,0))
    p=subprocess.Popen([launcher,*([] if setup else ['--ai-interface','terminal'])],stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True);os.close(slave);screen=Screen(180,45)
    def read(seconds=.15):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            if select.select([master],[],[],.02)[0]:
                try:d=os.read(master,65536)
                except OSError as e:
                    if e.errno==errno.EIO:return
                    raise
                if d:screen.feed(d)
    def send(s):os.write(master,s.encode());read()
    def wait(s,seconds=12):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            read(.05)
            if s in screen.text():return
        raise AssertionError('Missing '+repr(s)+'\n'+screen.text())
    def point(text):
        rows=screen.text().splitlines();row=next(i for i,s in enumerate(rows) if text in s)
        return rows[row].index(text)+1,row+1
    def click(x,y):send(f'\x1b[<0;{x};{y}M\x1b[<0;{x};{y}m')
    try:
        wait('acme/atlas');send('\r');wait('Add command menu');send('\r');wait('acme/atlas #128')
        send('ga')
        if setup:wait('Connect Claude Code');send('\r')
        wait('Claude native fixture');send('first prompt\r');wait('Native answer: first prompt')
        # Most humans click the prompt before typing. Plain clicks must focus
        # input; only a deliberate drag should enter transcript selection.
        x,y=point('Native answer: first prompt');click(x,y+1)
        send('after click\r');wait('Native answer: after click',3)
        # Selection still copies exactly, and an ordinary click resumes typing.
        x,y=point('Native answer: after click')
        send(f'\x1b[<0;{x};{y}M\x1b[<0;{x+12};{y}m');read(.3)
        assert clipboard.read_text()=='Native answer'
        click(x,y+1);send('after selection\r');wait('Native answer: after selection')
        send('\x1d');send('\\j\r');wait('[+ Claude]');send('ga');wait('Claude native fixture')
        # Explicitly restoring the companion must land in its input again.
        send('after unfold\r');wait('Native answer: after unfold',3)
        send('\x1d');send('\\j\r');wait('[+ Claude]');click(*point('[+ Claude]'));wait('Claude native fixture')
        send('mouse restore\r');wait('Native answer: mouse restore')
        send('\x1d');send('\\j\r');wait('[+ Claude]');send('\\\r');wait('Claude native fixture')
        send('keyboard restore\r');wait('Native answer: keyboard restore')
        send('\x1d');send('\t')
        send('tab focus\r');wait('Native answer: tab focus')
        send('\x1d');send('q');p.wait(timeout=8);assert p.returncode==0
        assert ('enabled: true' if setup or '--enabled' in sys.argv else 'enabled: false') in (config/'lazyreview/config.yml').read_text()
        print('PASS launcher → chooser → native: '+('setup' if setup else 'CLI override')+', click focus, drag copy, keyboard/mouse unfold, Tab typing; offline')
    finally:
        if p.poll() is None:p.terminate();p.wait(timeout=8)
        os.close(master)
