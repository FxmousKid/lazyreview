"""Opt-in native Grok smoke: isolated LazyReview state, synthetic PR, two model turns.

Only the generated test directory and its read-only Review Context tool are
approved through visible native prompts. User config and existing sessions are
unchanged. The chooser/content bridge is offline; real Grok owns its conversation.
"""
import argparse,errno,fcntl,json,os,pathlib,re,select,shlex,shutil,struct,subprocess,tempfile,termios,time
from vt_screen import Screen
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--live',action='store_true',help='Use the existing Grok login for two short model turns')
args=parser.parse_args()
if not args.live:parser.error('--live is required')
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'lazyreview'))
with tempfile.TemporaryDirectory(prefix='rw-grok-live-') as temp:
    td=pathlib.Path(temp);config=td/'config';state=td/'state';stub=td/'bin'
    stub.mkdir();(config/'lazyreview').mkdir(parents=True)
    node=shutil.which('node');wrapper=stub/'node'
    wrapper.write_text('#!/usr/bin/env python3\nimport os,sys\nargs=sys.argv[1:]\nif any(x.endswith("/bridge.mjs") for x in args):args=['+repr(str(root/'tests/native-entry-backend.mjs'))+']\nos.execv('+repr(node)+',['+repr(node)+',*args])\n');wrapper.chmod(0o700)
    cli=td/'grok';restore=('export XDG_CONFIG_HOME='+shlex.quote(os.environ['XDG_CONFIG_HOME'])) if 'XDG_CONFIG_HOME' in os.environ else 'unset XDG_CONFIG_HOME'
    cli.write_text('#!/bin/sh\n'+restore+'\nexec '+shlex.quote(shutil.which('grok') or 'grok')+' "$@"\n');cli.chmod(0o700)
    (config/'lazyreview/config.yml').write_text('ai:\n  provider: grok\n  enabled: true\n  interface: terminal\n  autoPrepare: false\n  profile: grok-live\n  model: default\n  command: '+json.dumps(str(cli))+'\n')
    env={**os.environ,'TERM':'xterm-256color','XDG_CONFIG_HOME':str(config),'XDG_STATE_HOME':str(state),'PATH':str(stub)+os.pathsep+os.environ['PATH']}
    def launch():
        master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',50,200,0,0))
        p=subprocess.Popen([binary,'--ai-provider','grok'],stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True);os.close(slave)
        return p,master,Screen(200,50)
    p,master,screen=launch()
    def read(seconds=.2):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            if select.select([master],[],[],.02)[0]:
                try:data=os.read(master,65536)
                except OSError as error:
                    if error.errno==errno.EIO:return
                    raise
                if data:screen.feed(data)
    def send(keys):os.write(master,keys.encode());read()
    def approve_test_prompt():
        text=screen.text()
        if 'Do you trust the contents of this directory?' in text:
            assert str(state) in text,'Refusing trust outside the generated test directory'
            send('y');return
        if 'Allow (Review) Review Context?' in text:
            assert re.search(r'3\s+\(●\) Yes',text),'Expected a one-off Yes selection'
            send('\r')
    def wait(check,label,timeout=120,approvals=False):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            read(.1)
            if approvals:approve_test_prompt()
            if check():return
            assert p.poll() is None,'LazyReview exited during '+label
        raise AssertionError('Timed out: '+label+'\n'+screen.text())
    def text(value):wait(lambda:value in screen.text(),value,30)
    def record():return json.loads(next(state.rglob('session.json')).read_text())
    def open_terminal():
        text('acme/atlas');send('\r');text('Add command menu');send('\r');text('acme/atlas #128')
        send('ga');read(3);send('\x1c');send('Z');send('i')
        wait(lambda:'Shift+Tab:mode' in screen.text() and '❯' in screen.text(),'Grok native prompt',approvals=True)
        assert 'always-approve' not in screen.text()
        # Click the last prompt, not a previous user message in scrollback.
        lines=screen.text().splitlines();row=max(i for i,line in enumerate(lines) if '❯' in line);column=lines[row].index('❯')+2
        send(f'\x1b[<0;{column};{row+1}M\x1b[<0;{column};{row+1}m')
    def finish():
        send('\x1d');send('q');read(.4);p.wait(timeout=10);assert p.returncode==0;os.close(master)
    try:
        open_terminal();session=record()['session'];marker='LAZYREVIEW_GROK_ENTRY_'+session[:8]
        send('Call review_context with this question. Which function is invoked inside CommandMenu useEffect when query or reset changes? Reply in one sentence, include and remember marker '+marker+'. Use no other tool.\r')
        wait(lambda:len(record().get('messages',[]))>=2,'first answer and Stop hook',approvals=True)
        answer=record()['messages'][-1]['content'];assert 'reset' in answer and marker in answer
        turn=json.loads(next(state.rglob('native-turn.json')).read_text());assert turn['delivered'] is True
        finish();p,master,screen=launch();open_terminal();assert record()['session']==session
        send('What review marker did I ask you to remember? Reply only with that marker.\r')
        wait(lambda:len(record().get('messages',[]))>=4,'resumed answer and Stop hook',approvals=True)
        assert record()['messages'][-1]['content'].strip()==marker
        finish();assert not list(state.rglob('lock'))
        print(json.dumps({'nativeGrok':True,'clickToType':True,'groundedViaReviewContext':True,'nativeResume':True,'stopHooks':True,'modelTurns':2,'githubCalls':0,'githubWrites':0}))
    finally:
        if p.poll() is None:
            try:finish()
            except Exception:p.terminate();p.wait(timeout=10)
