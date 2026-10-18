"""Opt-in real interactive Claude + public PR; two short turns, no GitHub writes.

Isolated LazyReview config/state. A wrapper restores Claude's normal XDG environment
so the isolated LazyReview config cannot hide the user's existing CLI login.
"""
import os,sys,json,re,subprocess,tempfile,pathlib,fcntl,termios,struct,select,time,errno,shlex
from vt_screen import Screen
assert '--live' in sys.argv,'--live uses the existing Claude account for two short turns'
root=pathlib.Path(__file__).resolve().parent.parent
binary=os.environ.get('LAZYREVIEW_BINARY',str(root/'tui/target/release/lazyreview'))
chooser='--chooser' in sys.argv
token=os.environ.get('GH_TOKEN') or subprocess.check_output(['gh','auth','token','--hostname','github.com'],text=True,stderr=subprocess.DEVNULL).strip()
with tempfile.TemporaryDirectory(prefix='rw-native-live-') as tmp:
 td=pathlib.Path(tmp);config=td/'config';state=td/'state';(config/'lazyreview').mkdir(parents=True)
 wrapper=td/'claude';wrapper.write_text('#!/bin/sh\n'+('export XDG_CONFIG_HOME='+shlex.quote(os.environ['XDG_CONFIG_HOME']) if 'XDG_CONFIG_HOME' in os.environ else 'unset XDG_CONFIG_HOME')+'\nexec claude "$@"\n');wrapper.chmod(0o700)
 (config/'lazyreview/config.yml').write_text('ai:\n  enabled: true\n  interface: terminal\n  autoPrepare: false\n  profile: native-live\n  command: '+json.dumps(str(wrapper))+'\n')
 clipboard=td/'clipboard';stub=td/'bin';stub.mkdir();copy=stub/'pbcopy';copy.write_text('#!/usr/bin/env python3\nimport sys,pathlib,os\npathlib.Path(os.environ["LAZYREVIEW_CLIP_TEST"]).write_text(sys.stdin.read())\n');copy.chmod(0o700)
 env={**os.environ,'PATH':str(stub)+os.pathsep+os.environ['PATH'],'TERM':'xterm-256color','GH_TOKEN':token,'XDG_CONFIG_HOME':str(config),'XDG_STATE_HOME':str(state),'LAZYREVIEW_CLIP_TEST':str(clipboard)}
 def launch():
  master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',50,200,0,0))
  args=['--ai-interface','terminal']+([] if chooser else ['--pr','https://github.com/primer/react/pull/8520'])
  p=subprocess.Popen([binary,*args],stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True);os.close(slave)
  return p,master,Screen(200,50)
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
 def send(text):os.write(master,text.encode());read()
 def wait_for(check,label,timeout=90):
  end=time.monotonic()+timeout
  while time.monotonic()<end:
   read(.05)
   if check():return
  raise AssertionError('Timed out: '+label+(' (private chooser content omitted)' if label=='chooser ready' else '\n'+screen.text()))
 def wait(text,timeout=90):wait_for(lambda:text in screen.text(),text,timeout)
 def record():return json.loads(next(state.rglob('session.json')).read_text())
 def completed(count):return len(record().get('messages',[]))>=count
 def open_terminal():
  if chooser:
   wait_for(lambda:any(text in screen.text() for text in ['Select a repository and Enter','No repositories found']),'chooser ready')
   send('o');send('https://github.com/primer/react/pull/8520\r')
  wait('primer/react #8520');send('ga');read(3);send('\x1c');send('Z');send('i')
  # Approve only this test's own generated private workspace, not repository code.
  deadline=time.monotonic()+30
  while time.monotonic()<deadline:
   read(.2)
   if 'Yes, I trust this folder' in screen.text():send('\x1b[B\r');read(2)
   if 'Claude Code v' in screen.text() and 'Ready' in screen.text():
    # Click the real native prompt before typing; this was the missing
    # interaction in the earlier live happy-path test.
    rows=screen.text().splitlines();row=next(i for i,l in enumerate(rows) if '❯' in l);col=rows[row].index('❯')+2
    send(f'\x1b[<0;{col};{row+1}M\x1b[<0;{col};{row+1}m')
    return
  raise AssertionError('Claude did not become ready\n'+screen.text())
 def finish():
  send('\x1d');send('q');read(.6);p.wait(timeout=10);assert p.returncode==0;os.close(master)
 try:
  open_terminal();print('Native CLI ready',flush=True);session=record()['session'];snapshot=json.loads(next(state.rglob('snapshot.json')).read_text())
  patch=next(f['patch'] for f in snapshot['files'] if f['filename']=='examples/codesandbox/package.json')
  version=re.search(r'^\+\s*"react":\s*"([^"]+)"',patch,re.M)[1]
  marker='LazyReview-NATIVE-8520-'+session[:8]
  send('Remember the review marker '+marker+'. What exact new react version does the patch specify for examples/codesandbox/package.json? Answer with that version and the marker in one short sentence.\r')
  wait_for(lambda:completed(2),'first successful native Stop hook',120)
  first=record()['messages'][-1]['content'];assert version in first and marker in first,first;print('Grounded first answer saved by Stop hook',flush=True)
  # Real native answer can be copied through parent-owned selection.
  read(.5);lines=screen.text().splitlines();row=next(i for i,l in enumerate(lines) if marker in l and 'Remember the review' not in l);col=lines[row].index(marker)
  send(f'\x1b[<0;{col+1};{row+1}M');send(f'\x1b[<0;{col+len(marker)};{row+1}m');read(.5)
  assert clipboard.read_text()==marker
  send('i');finish()
  p,master,screen=launch();open_terminal();assert record()['session']==session
  send('What review marker did I ask you to remember? Reply only with the marker.\r')
  wait_for(lambda:completed(4),'resumed successful native Stop hook',120)
  assert record()['messages'][-1]['content'].strip()==marker,record()['messages'][-1]
  send('/clear\r');wait_for(lambda:record()['session']!=session,'native /clear session adoption',30)
  cleared=record()['session'];assert record()['messages']==[]
  finish();p,master,screen=launch();open_terminal();assert record()['session']==cleared
  assert 'No conversation found' not in screen.text()
  finish();assert not list(state.rglob('lock'))
  print(json.dumps({'nativeClearResume':True,'realInteractiveCli':True,'nativeResume':True,'patchGrounding':True,'hookCompletion':True,'nativeAnswerCopied':True,'clickToType':True,'chooserLaunch':chooser,'turns':2,'githubWrites':0}))
 finally:
  if p.poll() is None:
   # Parent-controlled mouse input releases capture before orderly quit.
   send('\x1b[<0;2;4M\x1b[<0;2;4m');send('\x1b');send('q');read(.5)
   if p.poll() is None:p.terminate()
   p.wait(timeout=10)
