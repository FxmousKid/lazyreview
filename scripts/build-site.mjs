import {cp,mkdir,rm,writeFile,stat} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {join,extname} from 'node:path';
import {sample} from '../lib/sample.ts';
import {parsePatch,categorize} from '../lib/review.ts';
const root=fileURLToPath(new URL('../',import.meta.url));
export async function buildSite(){
 const out=join(root,'dist/site');await rm(out,{recursive:true,force:true});await mkdir(out,{recursive:true});
 const allowed=new Set(['.html','.css','.js','.json','.svg','.png','.jpg','.webp','.woff2','.ttf','.txt']);
 await cp(join(root,'site'),out,{recursive:true,filter:async p=>(await stat(p)).isDirectory()||allowed.has(extname(p))});
 const picks=['src/components/command-menu/use-keyboard-navigation.ts','src/components/command-menu/command-menu.tsx','src/components/command-menu/__tests__/command-menu.test.tsx','docs/components/command-menu.md'];
 const files=sample.files.filter(f=>picks.includes(f.filename)).map(f=>({...f,category:categorize(f.filename),lines:parsePatch(f.patch||'')}));
 if(files.length<3)throw Error('Showcase needs implementation, tests and docs sample files');
 const data={title:sample.title,owner:sample.owner,repo:sample.repo,number:sample.number,files};
 await writeFile(join(out,'sample-data.js'),'window.LR_SAMPLE='+JSON.stringify(data).replaceAll('<','\\u003c')+';\n');
 await cp(join(root,'demo/lazyreview-demo.mp4'),join(out,'assets/tour.mp4'));
 return out;
}
if(process.argv[1]===fileURLToPath(import.meta.url))console.log(await buildSite());
