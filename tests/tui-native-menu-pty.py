"""Chooser → native menu click, keyboard and text-copy replay; offline by default.

--live starts real Claude and clicks No only; it NEVER approves trust.
--baseline confirms the installed highlight-only failure after testing arrows/copying.
"""
import argparse,errno,fcntl,json,os,pathlib,select,shlex,shutil,struct,subprocess,tempfile,termios,time
from vt_screen import Screen as BaseScreen
class Screen(BaseScreen):
    """Strip Zellij's OSC/charset controls before the small Ratatui decoder."""
    def __init__(self,*args):super().__init__(*args);self.control_pending=b''
    def feed(self,data):
        data=self.control_pending+data;self.control_pending=b'';clean=bytearray();i=0
        while i<len(data):
            if data[i]==27:
                if i+1==len(data):break
                if data[i+1] in (ord(']'),ord('P'),ord('^'),ord('_')):
                    stop=data.find(b'\x1b\\',i+2);bell=data.find(b'\x07',i+2) if data[i+1]==ord(']') else -1
                    if bell>=0 and (stop<0 or bell<stop):i=bell+1;continue
                    if stop<0:break
                    i=stop+2;continue
                if data[i+1] in (ord('('),ord(')'),ord('*'),ord('+')):
                    if i+2==len(data):break
                    i+=3;continue
            clean.append(data[i]);i+=1
        self.control_pending=data[i:];super().feed(bytes(clean))
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--live',action='store_true');p.add_argument('--baseline',action='store_true');p.add_argument('--mouse',action='store_true');p.add_argument('--width',type=int,default=180);p.add_argument('--height',type=int,default=50);p.add_argument('--zellij',action='store_true');p.add_argument('--zellij-config',type=pathlib.Path);p.add_argument('--auto-prepare',action='store_true');p.add_argument('--first-click',action='store_true');p.add_argument('--open',choices=['keys','mouse'],default='keys');p.add_argument('--trusted-repl',type=pathlib.Path);p.add_argument('--drop-release',action='store_true');p.add_argument('--copy-first',action='store_true');args=p.parse_args()
if args.trusted_repl:
    assert args.live and not args.auto_prepare,'REPL probe requires --live and autoPrepare false'
if args.zellij_config:args.zellij=True
root=pathlib.Path(__file__).resolve().parent.parent;binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'lazyreview'))
with tempfile.TemporaryDirectory(prefix='lr-menu-',dir='/private/tmp') as tmp:
    td=pathlib.Path(tmp);config=td/'config';state=td/'state';stub=td/'bin';stub.mkdir();(config/'lazyreview').mkdir(parents=True)
    node=shutil.which('node');wrapper=stub/'node'
    wrapper.write_text('#!/usr/bin/env python3\nimport os,sys\nargs=sys.argv[1:]\nif any(x.endswith("/bridge.mjs") for x in args):args=['+repr(str(root/'tests/native-entry-backend.mjs'))+']\nos.execv('+repr(node)+',['+repr(node)+',*args])\n');wrapper.chmod(0o700)
    cli=root/'tests/claude-menu-fixture.py'
    if args.live:
        cli=td/'claude';restore='export XDG_CONFIG_HOME='+shlex.quote(os.environ['XDG_CONFIG_HOME']) if 'XDG_CONFIG_HOME' in os.environ else 'unset XDG_CONFIG_HOME'
        cli.write_text('#!/bin/sh\n'+restore+'\n'+(('cd '+shlex.quote(str(args.trusted_repl))+' || exit 90\n') if args.trusted_repl else '')+shlex.quote(shutil.which('claude') or 'claude')+' "$@"\nprintf \'%s\' "$?" > '+shlex.quote(str(td/'cli-exit'))+'\n');cli.chmod(0o700)
    (config/'lazyreview/config.yml').write_text('ai:\n  enabled: true\n  autoPrepare: '+('true' if args.auto_prepare else 'false')+'\n'+('  provider: grok\n  model: default\n' if args.mouse else '')+'  command: '+json.dumps(str(cli))+'\n')
    clipboard=td/'clipboard';copy=stub/'pbcopy';copy.write_text('#!/usr/bin/env python3\nimport sys,pathlib\npathlib.Path('+repr(str(clipboard))+').write_text(sys.stdin.read())\n');copy.chmod(0o700)
    env={**os.environ,'PATH':str(stub)+os.pathsep+os.environ['PATH'],'TERM':'xterm-256color','XDG_CONFIG_HOME':str(config),'XDG_STATE_HOME':str(state)}
    if args.mouse:env['LAZYREVIEW_MENU_MOUSE']='1'
    command=[binary,'--ai-interface','terminal']
    if args.zellij:
        env.pop('ZELLIJ',None);env.pop('ZELLIJ_SESSION_NAME',None);env['ZELLIJ_SOCKET_DIR']=str(td/'sockets');env['TMPDIR']=str(td)
        zconfig=td/'zellij.kdl';zconfig.write_text('mouse_mode true\npane_frames false\nshow_startup_tips false\nshow_release_notes false\nsession_serialization false\n')
        layout=td/'layout.kdl';layout.write_text('layout {\n    pane command='+json.dumps(binary)+' close_on_exit=true {\n        args \"--ai-interface\" \"terminal\"\n    }\n}\n')
        command=['zellij','--config',str(args.zellij_config or zconfig),'--data-dir',str(td/'zellij-data'),'--session','lr-'+td.name[-8:],'--new-session-with-layout',str(layout),'options','--session-serialization','false','--disable-session-metadata','true']
    master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',args.height,args.width,0,0));proc=subprocess.Popen(command,stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True,preexec_fn=(lambda: fcntl.ioctl(0,termios.TIOCSCTTY,0)) if args.zellij else None);os.close(slave);screen=Screen(args.width,args.height);raw=bytearray()
    def read(seconds=.15):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            if select.select([master],[],[],.02)[0]:
                try:data=os.read(master,65536)
                except OSError as e:
                    if e.errno==errno.EIO:return
                    raise
                if data:
                    raw.extend(data);screen.feed(data)
                    if args.zellij:
                        for query,response in [(b'\x1b[6n',b'\x1b[1;1R'),(b'\x1b[c',b'\x1b[?62;4;6;22c'),(b'\x1b]11;?\x07',b'\x1b]11;rgb:0d0d/1111/1717\x07')]:
                            if query in data:os.write(master,response)
    def send(keys):os.write(master,keys.encode());read()
    def quit_workbench():
        if 'Selecting text' in screen.text():send('\x1b')
        if 'Ctrl-] LazyReview' in screen.text():send('\x1d')
        send('q')
    def wait(text,timeout=20):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            read(.1)
            if text in screen.text():return
        raise AssertionError('Missing '+text+' returncode='+str(proc.poll())+' rawtail='+repr(bytes(raw[-600:]))+'\n'+screen.text())
    def point(label):
        rows=screen.text().splitlines();y=next(i for i,row in enumerate(rows) if label in row);prefix=rows[y][:rows[y].index(label)];return sum(2 if __import__('unicodedata').east_asian_width(c) in ('W','F') else 1 for c in prefix)+1,y+1
    def selected(label):
        rows=screen.text().splitlines();row=next(row for row in rows if label in row);return any(marker in row[:row.index(label)] for marker in ['❯','›','>'])
    def click(label):
        x,y=point(label);send(f'\x1b[<0;{x};{y}M\x1b[<0;{x};{y}m')
    def copy_text(label):
        x,y=point(label);send(f'\x1b[<0;{x};{y}M');send(f'\x1b[<32;{x+len(label)-1};{y}M');send(f'\x1b[<0;{x+len(label)-1};{y}m');read(.3)
        assert clipboard.read_text()==label,repr(clipboard.read_text())
        assert 'Copied selection' in screen.text()
    def record():return json.loads(next(state.rglob('fixture-menu.json')).read_text())
    try:
        wait('acme/atlas');send('\r');wait('#128');send('\r');wait('acme/atlas #128')
        if args.open=='mouse':click('[+ Claude]')
        else:send('ga')
        if args.trusted_repl:
            # Never approve a new trust boundary. This probe is useful only in a
            # folder the user has already trusted in Claude.
            end=time.monotonic()+30
            while time.monotonic()<end:
                read(.15)
                if 'Accessing workspace:' in screen.text() or 'Do you trust' in screen.text():
                    wait('No, exit');click('No, exit')
                    raise AssertionError('REPL probe aborted: this folder needs trust approval')
                if 'Claude Code v' in screen.text() and '❯' in screen.text():break
            else:raise AssertionError('No actual Claude prompt appeared\n'+screen.text())
            print('READY actual Claude REPL; no trust approved',flush=True)
            if args.copy_first:copy_text('Claude Code v')
            else:
                x,y=point('❯');send(f'\x1b[<0;{x+2};{y}M'+('' if args.drop_release else f'\x1b[<0;{x+2};{y}m'))
            marker='lazyreview-unsent-probe';send(marker);read(.5)
            assert marker in screen.text(),'Immediate typing did not reach actual Claude\n'+screen.text()
            send('\x7f'*5);read(.3);assert marker not in screen.text() and marker[:-5] in screen.text(),'Backspace did not edit actual Claude prompt'
            send('\x1d');send('\t');send('-tab-focus');read(.5)
            assert '-tab-focus' in screen.text(),'Tab back to companion did not restore typing\n'+screen.text()
            send('\x15');send('\x03');read(.3)
            print('PASS actual trusted REPL: '+('copy then immediate ' if args.copy_first else 'missing-release recovery, ' if args.drop_release else 'first-click ')+ 'type, Backspace, pane Tab return, clear; no prompt submitted.')
            quit_workbench();proc.wait(timeout=8);assert proc.returncode==0
            raise SystemExit(0)
        wait('Yes, I trust');wait('cancel');read(.3)
        assert selected('No, exit')
        if args.first_click or args.copy_first:
            if args.copy_first:
                copy_text('Accessing workspace:' if 'Accessing workspace:' in screen.text() else 'No, exit');send('\r')
            elif args.drop_release:
                x,y=point('No, exit');send(f'\x1b[<0;{x};{y}M');send('\r')
            else:click('No, exit' if args.live else 'Yes, I trust')
            read(.5)
            if args.live:
                end=time.monotonic()+10
                while not (td/'cli-exit').exists() and time.monotonic()<end:read(.1)
                assert (td/'cli-exit').exists(),'Native No choice did not exit Claude\n'+screen.text()
            else:assert record()['confirmed']==([0] if args.drop_release or args.copy_first else [1]),record()
            print('PASS '+('copy then immediate Enter on No' if args.copy_first else 'missing mouse release then explicit Enter on No' if args.drop_release else 'first interaction is a click')+'; '+args.open+' companion entry; no real trust approved.')
            quit_workbench();proc.wait(timeout=8);assert proc.returncode==0
            raise SystemExit(0)
        send('\x1b[B');read(2);assert selected('Yes, I trust'),'Native Down did not select Yes\n'+screen.text()
        send('\x1b[A');read(.5);assert selected('No, exit'),'Native Up did not select No'
        # Exact rendered trust text is selected and copied, never a browser or
        # host-terminal selection which could include neighboring panes.
        copy_label='Accessing workspace:' if 'Accessing workspace:' in screen.text() else 'No, exit'
        x,y=point(copy_label);send(f'\x1b[<0;{x};{y}M');send(f'\x1b[<32;{x+len(copy_label)-1};{y}M');send(f'\x1b[<0;{x+len(copy_label)-1};{y}m');read(.3)
        assert clipboard.read_text()==copy_label,repr(clipboard.read_text())
        assert 'Copied selection' in screen.text()
        if not args.live:assert record()['mouse']==[],'Dragging text sent a click to the CLI'
        if not args.baseline and not args.mouse:
            # Copy must keep keyboard ownership in the CLI; arrows immediately
            # reach the visible selector without an extra mode-switch key.
            send('\x1b[B');read(.3);assert selected('Yes, I trust')
            send('\x1b[A');read(.3);assert selected('No, exit')
        target='No, exit' if args.live else 'Yes, I trust'
        if args.live:
            send('\x1b[B');read(.3);assert selected('Yes, I trust')
        x,y=point(target);send(f'\x1b[<0;{x};{y}M');read(.2)
        if not args.live:
            assert record()['confirmed']==[],'Mouse-down activated a child control'
            assert record()['mouse']==[],'Mouse-down was sent before a completed click'
        send(f'\x1b[<0;{x};{y}m');read(.5)
        if args.baseline:
            assert selected(target),'Baseline differs: click did not even select option\n'+screen.text()
            click(target);read(.3)
            if args.live:assert not (td/'cli-exit').exists(),'Expected native click to leave the trust menu open'
            else:assert record()['confirmed']==[],'Expected baseline highlight without confirmation'
            print('BASELINE: clicks only highlight; repeating click on selected option still does not choose it.')
        elif args.live:
            end=time.monotonic()+10
            while not (td/'cli-exit').exists() and time.monotonic()<end:read(.1)
            assert (td/'cli-exit').exists(),'Click No did not exit Claude\n'+screen.text()
            print('PASS actual Claude: No click exits; arrows and exact text copy work; trust never approved.')
        else:
            assert record()['confirmed']==[1],record()
            click('Yes, I trust');read(.3);assert record()['confirmed']==[1,1],record()
            click('No, exit');read(.3);assert record()['confirmed']==[1,1,0],record()
            # Drag directly across choice labels: selection must not choose.
            x,y=point('No, exit');x2,y2=point('Yes, I trust')
            send(f'\x1b[<0;{x};{y}M\x1b[<32;{x2+8};{y2}M\x1b[<0;{x2+8};{y2}m');read(.3)
            saved=record();assert saved['confirmed']==[1,1,0],saved
            assert len(saved['mouse'])==(6 if args.mouse else 0),saved
            print('PASS fixture: Yes/No clicks activate, repeat selected click activates, drag never activates; '+('SGR child' if args.mouse else 'keyboard-only child'))
        quit_workbench();proc.wait(timeout=8);assert proc.returncode==0
    finally:
        if args.zellij:
            subprocess.run(['zellij','kill-session','lr-'+td.name[-8:]],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5)
        if proc.poll() is None:
            try:quit_workbench();read(.3)
            except OSError:pass
            try:proc.wait(timeout=8)
            except subprocess.TimeoutExpired:proc.kill();proc.wait(timeout=3)
        os.close(master)
