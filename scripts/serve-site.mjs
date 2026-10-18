import {createServer} from 'node:http';
import {stat} from 'node:fs/promises';
import {createReadStream} from 'node:fs';
import {resolve,extname,sep} from 'node:path';
import {buildSite} from './build-site.mjs';
const root=await buildSite();
const types={'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.svg':'image/svg+xml','.webp':'image/webp','.png':'image/png','.ttf':'font/ttf','.txt':'text/plain; charset=utf-8','.mp4':'video/mp4'};
const server=createServer(async(req,res)=>{
 try{
  const url=new URL(req.url,'http://localhost');
  if(url.pathname==='/'||url.pathname==='/lazyreview'){res.writeHead(308,{location:'/lazyreview/'});res.end();return;}
  if(!url.pathname.startsWith('/lazyreview/'))throw Error('missing');
  let file=resolve(root,decodeURIComponent(url.pathname.slice('/lazyreview/'.length))||'index.html');
  if(!file.startsWith(root+sep))throw Error('invalid path');
  let info=await stat(file);if(info.isDirectory()){file=resolve(file,'index.html');info=await stat(file);}
  const headers={'content-type':types[extname(file)]||'application/octet-stream','cache-control':'no-store','x-content-type-options':'nosniff','accept-ranges':'bytes'};
  const range=req.headers.range?.match(/^bytes=(\d+)-(\d*)$/);
  if(range){const start=Number(range[1]),end=Math.min(Number(range[2]||info.size-1),info.size-1);if(start>end){res.writeHead(416,{'content-range':`bytes */${info.size}`});res.end();return;}res.writeHead(206,{...headers,'content-range':`bytes ${start}-${end}/${info.size}`,'content-length':end-start+1});createReadStream(file,{start,end}).pipe(res);}
  else{res.writeHead(200,{...headers,'content-length':info.size});if(req.method==='HEAD')res.end();else createReadStream(file).pipe(res);}
 }catch{res.writeHead(404);res.end('Not found');}
});
server.listen(Number(process.env.PORT||4179),'127.0.0.1',()=>console.log(`http://127.0.0.1:${server.address().port}/lazyreview/`));
