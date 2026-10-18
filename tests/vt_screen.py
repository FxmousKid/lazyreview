"""Small in-memory screen decoder for Ratatui PTY assertions; never logs terminal data."""
import codecs
import re
import unicodedata
class Screen:
    def __init__(self, width=150, height=45):
        self.width=width; self.height=height; self.x=0; self.y=0
        self.rows=[[' ']*width for _ in range(height)]
        self.decoder=codecs.getincrementaldecoder('utf8')('replace'); self.pending=''
    def feed(self, data):
        self.pending += self.decoder.decode(data)
        while self.pending:
            if self.pending.startswith('\x1b['):
                match=re.match(r'\x1b\[([0-?]*)([ -/]*)([@-~])',self.pending)
                if not match: return
                raw,_,code=match.groups(); self.pending=self.pending[match.end():]
                try: args=[int(p) if p else 0 for p in raw.lstrip('?<>').split(';')]
                except ValueError: args=[0]
                n=args[0] or 1
                if code in ('H','f'): self.y=max(0,(args[0] or 1)-1); self.x=max(0,((args[1] if len(args)>1 else 1)or 1)-1)
                elif code=='A': self.y=max(0,self.y-n)
                elif code=='B': self.y=min(self.height-1,self.y+n)
                elif code=='C': self.x=min(self.width-1,self.x+n)
                elif code=='D': self.x=max(0,self.x-n)
                elif code=='G': self.x=n-1
                elif code=='d': self.y=n-1
                elif code=='J':
                    if args[0] in (2,3): self.rows=[[' ']*self.width for _ in range(self.height)]
                    elif args[0]==0:
                        self.rows[self.y][self.x:]=[' ']*(self.width-self.x)
                        for y in range(self.y+1,self.height): self.rows[y]=[' ']*self.width
                elif code=='K':
                    start,end=(0,self.width) if args[0]==2 else (0,self.x+1) if args[0]==1 else (self.x,self.width)
                    self.rows[self.y][start:end]=[' ']*(end-start)
                continue
            if self.pending.startswith('\x1b'):
                if len(self.pending)<2:return
                self.pending=self.pending[2:];continue
            c=self.pending[0];self.pending=self.pending[1:]
            if c=='\r':self.x=0;continue
            if c=='\n':self.y=min(self.height-1,self.y+1);continue
            if c=='\b':self.x=max(0,self.x-1);continue
            if c=='\t':self.x=min(self.width-1,(self.x//8+1)*8);continue
            if ord(c)<32 or unicodedata.combining(c):continue
            width=2 if unicodedata.east_asian_width(c) in ('W','F') else 1
            if self.x>=self.width:self.x=0;self.y=min(self.height-1,self.y+1)
            self.rows[self.y][self.x]=c
            if width==2 and self.x+1<self.width:self.rows[self.y][self.x+1]=''
            self.x+=width
    def text(self): return '\n'.join(''.join(row) for row in self.rows)
