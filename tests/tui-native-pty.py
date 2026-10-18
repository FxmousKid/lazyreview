"""Real public PR reads + local interactive CLI fixture; no model or GitHub writes."""
import os,sys,json,subprocess,tempfile,pathlib,fcntl,termios,struct,select,time,errno
from vt_screen import Screen
assert '--live' in sys.argv,'--live enables read-only public GitHub loading'
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/debug/lazyreview'))
token=os.environ.get('GH_TOKEN') or subprocess.check_output(['gh','auth','token','--hostname','github.com'],text=True,stderr=subprocess.DEVNULL).strip()
with tempfile.TemporaryDirectory(prefix='rw-native-pty-') as td:
    td=pathlib.Path(td);config=td/'config';state=td/'state';(config/'lazyreview').mkdir(parents=True)
    (config/'lazyreview/config.yml').write_text('ai:\n  enabled: true\n  interface: terminal\n  autoPrepare: false\n  profile: native-fixture\n  command: '+json.dumps(str(root/'tests/claude-terminal-fixture.py'))+'\n')
    env={**os.environ,'TERM':'xterm-256color','GH_TOKEN':token,'XDG_CONFIG_HOME':str(config),'XDG_STATE_HOME':str(state)}
    def launch():
        master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',45,180,0,0))
        p=subprocess.Popen([binary,'--pr','https://github.com/primer/react/pull/8520'],stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True);os.close(slave)
        return p,master,Screen(180,45)
    p,master,screen=launch()
    def read(seconds=.3):
        until=time.monotonic()+seconds
        while time.monotonic()<until:
            if select.select([master],[],[],.02)[0]:
                try:d=os.read(master,65536)
                except OSError as e:
                    if e.errno==errno.EIO:return
                    raise
                if d:screen.feed(d)
    def wait(text,timeout=45):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            read(.05)
            if text in screen.text():return
        raise AssertionError('Missing '+text+'\n'+screen.text())
    def send(text):os.write(master,text.encode());read()
    def finish():
        send('\x1dq');read(.5);p.wait(timeout=8);assert p.returncode==0;os.close(master)
    try:
        wait('primer/react #8520');send('ga');wait('Claude native fixture');wait('New native session')
        send('q stays in Claude\r');wait('Native answer: q stays');assert p.poll() is None
        records=list(state.rglob('session.json'));assert len(records)==1;record=json.loads(records[0].read_text());session=record['session'];assert record['started']
        context=json.loads((records[0].parent/'fixture-context.json').read_text());assert 'Review evidence' in context['hookSpecificOutput']['additionalContext']
        send('\x1c');send('Z');read(.3);size=json.loads((records[0].parent/'fixture-size.json').read_text());assert size[0]>=170, size;send('=')
        send('i');send('second question\r');wait('Native answer: second question')
        finish();p,master,screen=launch();wait('primer/react #8520');send('ga');wait('Resumed native session')
        assert json.loads(records[0].read_text())['session']==session
        send('\x1c');send('N');wait('New native session');read(.3);assert json.loads(records[0].read_text())['session']!=session
        send('i');finish();assert not list(state.rglob('lock'))
        print('PASS native PTY: real child TTY, keystrokes, context hook, resize, native ID resume/new, owned shutdown')
    finally:
        if p.poll() is None:p.terminate();p.wait(timeout=6)
