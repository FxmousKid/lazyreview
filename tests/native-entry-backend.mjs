// Offline discovery/content fixture. The application and its native CLI bridge
// still execute normally; only GitHub-facing operations are replaced.
import {createInterface} from 'node:readline';
import {sample} from '../lib/sample.ts';
import {categorize,parsePatch,splitRows} from '../lib/review.ts';
const pr={...structuredClone(sample),demo:false,url:'https://github.com/acme/atlas/pull/128'};
pr.files=pr.files.map(f=>({...f,category:categorize(f.filename),lines:parsePatch(f.patch),splitRows:splitRows(parsePatch(f.patch))}));
for await(const line of createInterface({input:process.stdin})){
  const input=JSON.parse(line);
  let data;
  switch(input.operation){
    case 'repositories':data={repositories:[{name:'acme/atlas',openPrs:1,commits:4}],viewer:'you',total:1};break;
    case 'discover':data={items:[{repository:'acme/atlas',number:128,title:pr.title,author:'you',url:pr.url}],viewer:'you',total:1,page:1};break;
    case 'load':data=pr;break;
    case 'syntax':{
      const file=pr.files.find(f=>f.filename===input.file.filename);
      data={left:file?.originalContent??null,right:file?.content??null,leftPath:input.file.filename,rightPath:input.file.filename};break;
    }
    default:throw Error('Unexpected offline fixture operation: '+input.operation);
  }
  console.log(JSON.stringify({ok:true,data}));
}
