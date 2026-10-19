// A bounded pool of private, sample-only native app processes. No shell or PTY.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,dirname,resolve} from 'node:path';
import {randomBytes} from 'node:crypto';
import {existsSync} from 'node:fs';
import {gzipSync} from 'node:zlib';
import {fileURLToPath} from 'node:url';
const ROOT=fileURLToPath(new URL('../',import.meta.url));
export function nativeBinary(){return process.env.LAZYREVIEW_DEMO_BINARY||[join(ROOT,'tui/target/release/lazyreview'),join(ROOT,'bin/lazyreview')].find(existsSync);}
class Session {
  constructor(child,home){
    this.child=child;this.home=home;this.pending=null;this.closed=false;this.queue=Promise.resolve();this.touched=Date.now();this.created=this.touched;this.count=0;this.window=this.created;
    const lines=createInterface({input:child.stdout});
    lines.on('line',line=>{const p=this.pending;if(!p)return;this.pending=null;clearTimeout(p.timer);try{p.resolve(JSON.parse(line));}catch{p.reject(Error('Invalid native response'));}});
    child.stdin.on('error',()=>this.fail());
    child.once('error',()=>{this.fail();this.closed=true;void rm(home,{recursive:true,force:true});});child.once('exit',()=>{this.closed=true;this.fail();void rm(home,{recursive:true,force:true});});
    // Stderr is intentionally discarded; never expose native paths/process details.
  }
  fail(){if(this.pending){clearTimeout(this.pending.timer);this.pending.reject(Error('Demo session stopped. Reset to start again.'));this.pending=null;}}
  update(events){
    const now=Date.now();if(events.some(event=>event?.type!=='poll'))this.touched=now;if(now-this.window>1000){this.window=now;this.count=0;}
    if(++this.count>100)return Promise.reject(Error('Too many input requests.'));
    const run=()=>new Promise((resolve,reject)=>{
      if(this.closed){reject(Error('Session expired. Reset to start again.'));return;}
      const timer=setTimeout(()=>{this.pending=null;reject(Error('Demo response timed out.'));this.close();},10000);
      this.pending={resolve,reject,timer};this.child.stdin.write(JSON.stringify({events})+'\n',error=>{if(error)this.fail();});
    });
    const result=this.queue.then(run);this.queue=result.catch(()=>{});return result;
  }
  close(){
    if(this.closed)return;this.closed=true;this.fail();this.child.stdin.end();
    // EOF normally drops both private bridge children. Reap the entire isolated
    // process group if a worker stalls; no other terminal shares that group.
    const timer=setTimeout(()=>{try{process.kill(-this.child.pid,'SIGKILL');}catch{}},1000);timer.unref();
    this.child.once('exit',()=>clearTimeout(timer));
  }
}
export class TuiSessions {
  constructor({binary=nativeBinary(),max=8,idleMs=300000,lifetimeMs=1800000}={}){
    this.binary=binary;this.max=max;this.idleMs=idleMs;this.lifetimeMs=lifetimeMs;this.sessions=new Map();this.creations=[];this.starting=0;this.closed=false;
    this.timer=setInterval(()=>this.sweep(),15000);this.timer.unref();
  }
  sweep(){const now=Date.now();for(const [id,s] of this.sessions)if(s.closed||now-s.touched>this.idleMs||now-s.created>this.lifetimeMs){s.close();this.sessions.delete(id);}}
  async create(cols,rows){
    if(this.closed)throw Error('Demo has stopped. Try again shortly.');
    this.sweep();if(!this.binary||!existsSync(this.binary))throw Error('The interactive demo is unavailable. Try again shortly.');
    this.creations=this.creations.filter(t=>Date.now()-t<60000);
    if(this.sessions.size+this.starting>=this.max||this.creations.length>=30)throw Error('Demo is busy. Try again shortly.');
    this.creations.push(Date.now());
    this.starting++;
    const id=randomBytes(24).toString('hex');let home,session;
    try{
      home=await mkdtemp(join(tmpdir(),'lazyreview-browser-'));
      // close() may run while the private directory is being created. Do not
      // launch a detached child after teardown has already drained the pool.
      if(this.closed)throw Error('Demo has stopped. Try again shortly.');
      // Explicit environment: no user GitHub tokens, provider keys, configs, SSH,
      // shell initialization or native CLI credentials cross into a visitor session.
      const env={PATH:dirname(process.execPath),HOME:home,XDG_CONFIG_HOME:join(home,'config'),XDG_CACHE_HOME:join(home,'cache'),TMPDIR:home,LANG:'en_US.UTF-8'};
      const child=spawn(resolve(this.binary),['--browser-demo'],{cwd:home,env,detached:true,stdio:['pipe','pipe','ignore']});
      session=new Session(child,home);this.sessions.set(id,session);
    }catch(error){if(home)await rm(home,{recursive:true,force:true});throw error;}
    finally{this.starting--;}
    try{
      const frame=await session.update([{type:'resize',cols,rows}]);
      if(this.closed)throw Error('Demo has stopped. Try again shortly.');
      return {session:id,...frame};
    }
    catch(error){session.close();this.sessions.delete(id);throw error;}
  }
  async handle(req,res){
    if(new URL(req.url,'http://localhost').pathname!=='/lazyreview/demo/api')return false;
    const reply=(status,body)=>{
      const raw=Buffer.from(JSON.stringify(body));const zipped=raw.length>1000&&/\bgzip\b/.test(req.headers['accept-encoding']||'');const bytes=zipped?gzipSync(raw):raw;
      res.writeHead(status,{'content-type':'application/json','cache-control':'no-store','x-content-type-options':'nosniff','vary':'Accept-Encoding',...(zipped?{'content-encoding':'gzip'}:{}),'content-length':bytes.length});res.end(bytes);
    };
    if(req.method!=='POST'||!req.headers['content-type']?.startsWith('application/json')){reply(405,{error:'JSON POST required'});return true;}
    // Reject cross-site browser requests. No CORS, cookies or ambient authority.
    if(req.headers.origin){try{if(new URL(req.headers.origin).host!==req.headers.host)throw Error();}catch{reply(403,{error:'Same-origin request required'});return true;}}
    try{
      let length=0;const chunks=[];
      for await(const chunk of req){length+=chunk.length;if(length>32768){reply(413,{error:'Input too large'});return true;}chunks.push(chunk);}
      const input=JSON.parse(Buffer.concat(chunks).toString());
      if(input.action==='create'){
        if(!Number.isInteger(input.cols)||input.cols<40||input.cols>220||!Number.isInteger(input.rows)||input.rows<16||input.rows>70)throw Error('Invalid terminal size');
        reply(200,await this.create(input.cols,input.rows));return true;
      }
      if(typeof input.session!=='string'||!/^[a-f0-9]{48}$/.test(input.session))throw Error('Invalid session');
      const s=this.sessions.get(input.session);if(!s||s.closed){reply(410,{error:'Demo expired. Reset to start again.'});return true;}
      if(input.action==='close'){s.close();this.sessions.delete(input.session);reply(200,{ok:true});return true;}
      if(!Array.isArray(input.events)||input.events.length>64)throw Error('Expected at most 64 input events');
      const frame=await s.update(input.events);reply(frame.ok?200:400,frame);
    }catch(error){reply(400,{error:error.message||'Demo unavailable'});}
    return true;
  }
  close(){this.closed=true;clearInterval(this.timer);for(const s of this.sessions.values())s.close();this.sessions.clear();}
}
