"""Isolated Claude UI integration: real public PR reads, deterministic local CLI.

Requires --live for GitHub access; never invokes real Claude or mutates GitHub.
"""
import argparse, errno, fcntl, json, os, pathlib, select, struct, subprocess, tempfile, termios, time
from vt_screen import Screen
parser=argparse.ArgumentParser();parser.add_argument('--live',action='store_true');args=parser.parse_args()
if not args.live:parser.error('--live enables read-only public GitHub loading')
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/release/lazyreview'))
token=os.environ.get('GH_TOKEN') or subprocess.check_output(['gh','auth','token','--hostname','github.com'],text=True,stderr=subprocess.DEVNULL).strip()
with tempfile.TemporaryDirectory(prefix='rw-ai-pty-') as temp:
    state=pathlib.Path(temp)/'state'; config=pathlib.Path(temp)/'config';(config/'lazyreview').mkdir(parents=True)
    path=config/'lazyreview/config.yml'
    path.write_text('gui:\n  themePreset: blue\nai:\n  enabled: false\n  profile: fixture\n  command: '+json.dumps(str(root/'tests/claude-fixture.mjs'))+'\n  model: fixture\n  autoPrepare: true\n  maxBudgetUsd: 1\n')
    stub=pathlib.Path(temp)/'bin';stub.mkdir()
    opener=stub/'open';opener.write_text('#!/usr/bin/env python3\nimport os,sys,pathlib\npathlib.Path(os.environ["LAZYREVIEW_OPEN_LOG"]).write_text(sys.argv[1])\n');opener.chmod(0o700)
    opened=pathlib.Path(temp)/'opened.txt'
    env={**os.environ,'PATH':str(stub)+os.pathsep+os.environ['PATH'],'LAZYREVIEW_OPEN_LOG':str(opened),'GH_TOKEN':token,'XDG_CONFIG_HOME':str(config),'XDG_STATE_HOME':str(state),'TERM':'xterm-256color','COLORTERM':'truecolor'}
    def launch():
        master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',45,150,0,0))
        proc=subprocess.Popen([binary,'--pr','https://github.com/primer/react/pull/8520'],stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True);os.close(slave)
        return proc,master,Screen(),bytearray()
    proc,master,screen,seen=launch()
    def wait(text,timeout=45):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            ready,_,_=select.select([master],[],[],.02)
            if ready:
                try:data=os.read(master,65536)
                except OSError as e:
                    if e.errno==errno.EIO:break
                    raise
                if not data:break
                seen.extend(data);screen.feed(data)
            if text in screen.text():return
        raise AssertionError('Missing screen state: '+text+'; companion: '+repr('\n'.join(line[95:] for line in screen.text().splitlines())))
    def send(text):os.write(master,text.encode())
    def finish():
        send('q')
        deadline=time.monotonic()+12
        # Drain terminal output during shutdown; a full PTY can block the final frame.
        while proc.poll() is None and time.monotonic()<deadline:
            ready,_,_=select.select([master],[],[],.05)
            if ready:
                try:
                    data=os.read(master,65536)
                    if data:seen.extend(data);screen.feed(data)
                except OSError as error:
                    if error.errno!=errno.EIO:raise
        proc.wait(timeout=1)
        assert proc.returncode==0
        os.close(master)
    try:
        wait('primer/react #8520');send('ga');wait('Connect Claude Code');send('\r');wait('Claude · Ready')
        assert 'themePreset: blue' in path.read_text()
        send('i');wait('Ask Claude');send('Explain this change\r');wait('▎ You');wait('Claude · Ready');wait('A streamed answer.')
        # The question does not need a manually attached file.
        assert 'Explain this change' in screen.text()
        files=list(state.glob('lazyreview/claude/*/session.json'));assert len(files)==1
        record=json.loads(files[0].read_text());first=record['session'];assert len(record['messages'])==2
        send('i');wait('Ask Claude');send('\x15SIMULATE_ARTIFACT\r');wait('Fixture diagram');wait('Claude · Ready')
        send('go');wait('Artifacts · 1');send('\r');wait('Opened Fixture diagram')
        assert opened.exists() and pathlib.Path(opened.read_text()).name=='preview.html'
        assert pathlib.Path(opened.read_text()).is_relative_to(state)
        send('\x1b');time.sleep(.2) # Distinguish Escape from an Alt-i key sequence.
        send('i');wait('Ask Claude');send('\x15SIMULATE_SLOW\r');send('X');wait('Stopped')
        finish();proc,master,screen,seen=launch();wait('primer/react #8520');send('ga');wait('Explain this change')
        assert json.loads(files[0].read_text())['session']==first
        send('N')
        # Ready may still be the previous rendered frame; await persisted identity.
        deadline=time.monotonic()+10
        while json.loads(files[0].read_text())['session']==first and time.monotonic()<deadline:
            ready,_,_=select.select([master],[],[],.05)
            if ready:screen.feed(os.read(master,65536))
        assert json.loads(files[0].read_text())['session']!=first
        wait('Claude · Ready')
        send('go');wait('Artifacts · 1');wait('Fixture diagram');send('\x1b');time.sleep(.2)
        finish()
        assert not list(state.rglob('lock'))
        print('PASS Claude terminal: setup preserves theme, background preparation, attachment-free chat, streamed answer, stop, restart/resume, new chat, artifact create/open/persistence, clean shutdown')
    finally:
        if proc.poll() is None:proc.terminate();proc.wait(timeout=6)
