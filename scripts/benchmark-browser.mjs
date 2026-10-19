// Repeatable work-count benchmark; timings intentionally have no machine-specific gate.
import {openBrowser,wait} from './browser-harness.mjs';
import {paintInstrumentation} from './browser-metrics.mjs';
const browser=await openBrowser(process.argv[2]);
const {cdp,evaluate,key}=browser;
try{
 await cdp('Page.addScriptToEvaluateOnNewDocument',{source:paintInstrumentation});
 await cdp('Page.navigate',{url:new URL('demo/',browser.url).href});
 for(let i=0;i<150;i++){if(await evaluate('!!window.lazyreviewTui?.getState().ready'))break;await wait(40);}
 if(!await evaluate('!!window.lazyreviewTui?.getState().ready'))throw Error('Native demo did not start');
 for(let i=0;i<150;i++){if(await evaluate('__paintStats.emptyFrames>=2'))break;await wait(40);}
 if(!await evaluate('__paintStats.emptyFrames>=2'))throw Error('Native rendering did not settle');
 await evaluate('Object.keys(__paintStats).forEach(k=>__paintStats[k]=0)');await wait(3200);
 const idle=await evaluate('({...__paintStats})');
 await evaluate('Object.keys(__paintStats).forEach(k=>__paintStats[k]=0)');
 for(let i=0;i<12;i++){await key(i%2?'k':'j');await wait(70);}await wait(100);
 const navigation=await evaluate('({...__paintStats})');
 console.log(JSON.stringify({idle,navigation},null,2));
}finally{await browser.close();}
