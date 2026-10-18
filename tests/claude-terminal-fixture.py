#!/usr/bin/env python3
"""Interactive PTY fixture. Exercises public hooks; no model/network."""
import os,sys,json,subprocess,signal,tty,termios
from pathlib import Path
args=sys.argv[1:]
def arg(name):return args[args.index(name)+1]
session=arg('--resume') if '--resume' in args else arg('--session-id')
settings=json.loads(arg('--settings'))
def hook(kind,**fields):
    command=settings['hooks'][kind][0]['hooks'][0]['command']
    payload={'hook_event_name':kind,'session_id':session,**fields}
    result=subprocess.run(command,shell=True,input=json.dumps(payload),capture_output=True,text=True)
    if result.returncode: raise RuntimeError(result.stderr)
    return json.loads(result.stdout or '{}')
def out(text):os.write(1,text.encode())
def stop(*_):
    hook('SessionEnd');sys.exit(0)
signal.signal(signal.SIGTERM,stop)
def resized(*_):
    size=os.get_terminal_size(1);Path('fixture-size.json').write_text(json.dumps([size.columns,size.lines]))
signal.signal(signal.SIGWINCH,resized)
assert os.isatty(0) and os.isatty(1)
old=termios.tcgetattr(0);tty.setraw(0)
try:
    hook('SessionStart');resized()
    out('\x1b[2J\x1b[H\x1b[?2004h\x1b[36mClaude native fixture\x1b[0m\r\n'+('Resumed native session' if '--resume' in args else 'New native session')+'\r\n> ')
    buf=bytearray()
    while True:
        b=os.read(0,1)
        if not b:break
        if b==b'\x03':buf.clear();out('\r\nInterrupted\r\n> ');continue
        if b==b'\r':
            prompt=buf.decode(errors='replace').replace('\x1b[200~','').replace('\x1b[201~','');buf.clear()
            result=hook('UserPromptSubmit',prompt=prompt)
            Path('fixture-context.json').write_text(json.dumps(result))
            out('\r\nNative answer: '+prompt+'\r\n> ')
            hook('Stop',last_assistant_message='Native answer: '+prompt)
        else:buf.extend(b);out(b.decode(errors='replace'))
finally:termios.tcsetattr(0,termios.TCSANOW,old)
