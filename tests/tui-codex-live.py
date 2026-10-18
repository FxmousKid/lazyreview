"""Interactive opt-in Codex PTY smoke against an offline synthetic PR.

LAZYREVIEW_BINARY=/path/to/lazyreview python3 tests/tui-codex-live.py --live

Uses a temporary CODEX_HOME with a private copy of the existing login. Never
changes the user's configuration or sessions. The temporary auth copy is deleted
on exit. Test state and public synthetic output remain in the printed directory.

Send one JSON action per line. No hooks are automatically trusted:
  {"keys":"\\u001b","wait":2}  # skip the native hook trust prompt
  {"keys":"Remember marker LazyReview-CODEX-128-M8. Call review_context and explain the count===0 guard.","wait":1}
  {"keys":"\\r","wait":20}
  {"record":true}
  {"restart":true,"record":true}
  {"keys":"\\u001b","wait":2}
  {"keys":"What exact review marker did I ask you to remember?","wait":1}
  {"keys":"\\r","wait":20}
  {"quit":true}

The CLI may interpret a fast burst as pasted text; submit after its paste hold.
Expected: review.review_context tool call, patch-grounded guard explanation,
identical LazyReview session UUID after restart, and remembered marker. With hooks
skipped the Paper message cache remains empty; that is intentional.
"""
import os,sys,json,subprocess,tempfile,pathlib,fcntl,termios,struct,select,time,errno,shlex,shutil,tomllib,atexit
assert '--live' in sys.argv, '--live uses your existing Codex account for the prompts you submit'
from vt_screen import Screen
root=pathlib.Path(__file__).resolve().parent.parent;td=pathlib.Path(tempfile.mkdtemp(prefix='rw-codex-live-'));config=td/'config';state=td/'state';stub=td/'bin';stub.mkdir();(config/'lazyreview').mkdir(parents=True)
node=shutil.which('node');wrapper=stub/'node';wrapper.write_text('#!/usr/bin/env python3\nimport os,sys\nargs=sys.argv[1:]\nif any(x.endswith("/bridge.mjs") for x in args):args=['+repr(str(root/'tests/native-entry-backend.mjs'))+']\nos.execv('+repr(node)+',['+repr(node)+',*args])\n');wrapper.chmod(0o700)
codexhome=td/'codex-home';codexhome.mkdir(mode=0o700)
auth=pathlib.Path(os.environ.get('CODEX_HOME',str(pathlib.Path.home()/'.codex')))/'auth.json'
atexit.register(lambda:(codexhome/'auth.json').unlink(missing_ok=True))
shutil.copyfile(auth,codexhome/'auth.json');(codexhome/'auth.json').chmod(0o600)
userconfig=auth.parent/'config.toml';local=tomllib.loads(userconfig.read_text()) if userconfig.exists() else {}
model=os.environ.get('LAZYREVIEW_CODEX_MODEL',local.get('model',''))
(codexhome/'config.toml').write_text(('model='+json.dumps(model)+'\n' if model else '')+'model_reasoning_effort="low"\ncli_auth_credentials_store="file"\n')
codex=td/'codex';codex.write_text('#!/bin/sh\nexport CODEX_HOME='+shlex.quote(str(codexhome))+'\nexec '+shlex.quote(shutil.which('codex'))+' "$@"\n');codex.chmod(0o700)
(config/'lazyreview/config.yml').write_text('ai:\n  provider: codex\n  enabled: true\n  interface: terminal\n  autoPrepare: false\n  profile: codex-live-probe\n  model: default\n  command: '+json.dumps(str(codex))+'\n')
env={**os.environ,'TERM':'xterm-256color','XDG_CONFIG_HOME':str(config),'XDG_STATE_HOME':str(state),'PATH':str(stub)+os.pathsep+os.environ['PATH']}
def launch():
 master,slave=os.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',50,200,0,0));p=subprocess.Popen([os.environ.get('LAZYREVIEW_BINARY',str(root/'lazyreview')),'--ai-provider','codex'],stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True);os.close(slave);return p,master,Screen(200,50)
p,master,screen=launch()
def read(seconds=.3):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  if select.select([master],[],[],.02)[0]:
   try:d=os.read(master,65536)
   except OSError as e:
    if e.errno==errno.EIO:return
    raise
   if d:screen.feed(d)
def send(s):os.write(master,s.encode());read()
def wait(s,timeout=20):
 end=time.monotonic()+timeout
 while time.monotonic()<end:
  read(.05)
  if s in screen.text():return
 raise AssertionError('Missing '+s+'\n'+screen.text())
def open_():
 wait('acme/atlas');send('\r');wait('Add command menu');send('\r');wait('acme/atlas #128');send('ga');read(3);send('\x1c');send('Z');send('i');read(3)
def finish():
 send('\x1d');send('q');read(.5);p.wait(timeout=10);os.close(master)
try:
 open_();print('STATE_DIR='+str(td),flush=True);print(screen.text(),flush=True)
 for line in sys.stdin:
  action=json.loads(line)
  if 'keys' in action:send(action['keys'])
  if 'wait' in action:read(action['wait'])
  if action.get('restart'):
   finish();p,master,screen=launch();open_()
  if action.get('record'):
   for path in state.rglob('session.json'):
    r=json.loads(path.read_text());print(json.dumps({k:r.get(k) for k in ['session','started','messages','revision']}),flush=True)
  if action.get('quit'):break
  print(screen.text(),flush=True)
finally:
 if p.poll() is None:
  try:finish()
  except:p.terminate();p.wait(timeout=10)
 (codexhome/'auth.json').unlink(missing_ok=True)
 print('probe ended, auth removed, state retained at '+str(td),flush=True)
