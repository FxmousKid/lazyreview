// Native CLI capabilities are explicit: never emulate a provider conversation.
export const PROVIDERS = Object.freeze({
  claude: {label:'Claude Code',identity:'claude-code',command:'claude',model:'sonnet',paper:true,context:'prompt hook',resume:'--resume UUID',tested:'2.1.302'},
  codex: {label:'Codex',identity:'codex',command:'codex',model:'default',paper:false,context:'review_context + hooks',resume:'resume UUID',tested:'0.163.0'},
  grok: {label:'Grok',identity:'grok',command:'grok',model:'default',paper:false,context:'review_context + hooks',resume:'--resume UUID',tested:'1.0.62'},
});
export function provider(config={}) {
  const id=config.provider||'claude';
  if(!Object.hasOwn(PROVIDERS,id))throw Error('ai.provider must be claude, codex or grok');
  return {id,...PROVIDERS[id]};
}
export function providerTable(){return Object.entries(PROVIDERS).map(([id,p])=>`| ${id} | ${p.label} | ${p.paper?'Paper, terminal':'terminal'} | ${p.resume} | ${p.context} | ${p.tested} |`).join('\n');}
