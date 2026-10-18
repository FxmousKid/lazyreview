import { selectedLines } from './review.ts';
// GitHub anchors one contiguous range. Extra pinned ranges are immutable links.
export function prepareComment(input, pr) {
  const ranges=input.ranges;
  if(!ranges?.length) return input;
  if(!Array.isArray(ranges)||ranges.length>20) throw new Error('Select at most 20 ranges.');
  for(const range of ranges) {
    const file=pr.files.find(f=>f.filename===range.path);
    if(!file||!['LEFT','RIGHT'].includes(range.side)||!Number.isInteger(range.start)||!Number.isInteger(range.end)||range.start<1||range.end<range.start) throw new Error('Invalid pinned range. Reselect the code.');
    const lines=selectedLines(file,range);
    if(lines.length!==range.end-range.start+1||new Set(lines.map(l=>l.hunk)).size!==1) throw new Error('Pinned code changed. Refresh and select the ranges again.');
  }
  const first=ranges[0];
  let body=String(input.body||'').trim();
  if(!body) throw new Error('Write a comment first.');
  if(ranges.length>1) {
    const refs=ranges.slice(1).map((r,i)=>{
      const file=pr.files.find(f=>f.filename===r.path);
      const repository=r.side==='LEFT'?(pr.baseRepo||`${pr.owner}/${pr.repo}`):(pr.headRepo||`${pr.owner}/${pr.repo}`);
      if(!/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error('Repository unavailable for a code reference.');
      const commit=r.side==='LEFT'?pr.base:pr.head;
      const path=r.side==='LEFT'?(file.previous_filename||r.path):r.path;
      const url=`https://github.com/${repository}/blob/${encodeURIComponent(commit)}/${path.split('/').map(encodeURIComponent).join('/')}#L${r.start}${r.end!==r.start?'-L'+r.end:''}`;
      return `- [Range ${i+2}, ${r.side==='LEFT'?'original':'modified'} lines ${r.start}–${r.end}](${url})`;
    });
    body+='\n\nRelated code:\n'+refs.join('\n');
  }
  return {...input,path:first.path,side:first.side,startLine:first.start,line:first.end,body};
}
