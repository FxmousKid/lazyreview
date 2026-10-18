#!/usr/bin/env python3
"""A keyboard-only trust menu like Claude's, or an SGR mouse-aware menu.

No authentication, model, GitHub or real workspace trust is involved.
"""
import json,os,re,signal,sys,termios,tty
from pathlib import Path
selected=0;confirmed=[];mouse=[];report=bool(os.environ.get('LAZYREVIEW_MENU_MOUSE'))
def save():Path('fixture-menu.json').write_text(json.dumps({'selected':selected,'confirmed':confirmed,'mouse':mouse}))
def draw():
    rows=['','Accessing workspace:','','Synthetic fixture directory','', 'Quick safety check: select an option.','','Security guide','',('❯ ' if selected==0 else '  ')+'No, exit',('❯ ' if selected==1 else '  ')+'Yes, I trust this folder','','Enter to confirm · Esc to cancel']
    os.write(1,('\x1b[2J\x1b[H'+'\r\n'.join(rows)).encode());save()
old=termios.tcgetattr(0);tty.setraw(0)
signal.signal(signal.SIGTERM,lambda *_:sys.exit(0))
try:
    os.write(1,b'\x1b[?1h'+(b'\x1b[?1000h\x1b[?1006h' if report else b''));draw()
    while True:
        b=os.read(0,1)
        if not b:break
        if b==b'\x1b':
            seq=b+os.read(0,2)
            if seq in (b'\x1b[A',b'\x1bOA'):selected=0;draw()
            elif seq in (b'\x1b[B',b'\x1bOB'):selected=1;draw()
            elif seq==b'\x1b[<':
                while seq[-1:] not in (b'M',b'm'):seq+=os.read(0,1)
                mouse.append(seq.decode());event=re.match(rb'\x1b\[<(\d+);(\d+);(\d+)([Mm])',seq)
                if event and seq[-1:]==b'm' and int(event[3]) in (10,11):selected=int(event[3])-10;confirmed.append(selected)
                draw()
        elif b==b'\r':confirmed.append(selected);draw()
        elif b==b'\x03':break
finally:termios.tcsetattr(0,termios.TCSANOW,old)
