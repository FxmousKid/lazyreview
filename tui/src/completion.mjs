// Read-only, repository-scoped mention/reference suggestions. No user content is logged.
export async function complete(input, token, fetcher=fetch) {
  const match=String(input.url||'').match(/^https:\/\/github\.com\/([\w.-]+)\/([\w.-]+)\/pull\/[1-9]\d*$/);
  if(!match) throw new Error('Open a GitHub PR first.');
  const [,owner,repo]=match, query=String(input.query||'');
  if(query.length>100||!['user','issue'].includes(input.kind)) throw new Error('Invalid suggestion query.');
  const result={url:input.url,kind:input.kind,query,id:input.id,items:[]};
  async function api(path,body) {
    let res;
    try {res=await fetcher('https://api.github.com'+path,{method:body?'POST':'GET',headers:{Accept:'application/vnd.github+json','X-GitHub-Api-Version':'2026-03-10',...(token?{Authorization:`Bearer ${token}`} : {}),...(body?{'Content-Type':'application/json'}:{})},...(body?{body:JSON.stringify(body)}:{}),signal:AbortSignal.timeout(8000)});}
    catch {throw new Error('Suggestions unavailable; continue typing.');}
    if(!res.ok) throw new Error('Suggestions unavailable; continue typing.');
    try{return await res.json();}catch{throw new Error('Suggestions unavailable; continue typing.');}
  }
  if(input.kind==='user') {
    if(!token) return result;
    const data=await api('/graphql',{query:'query($owner:String!,$repo:String!,$query:String!) { repository(owner:$owner,name:$repo) { mentionableUsers(first:15,query:$query) { nodes { login name } } } }',variables:{owner,repo,query}});
    if(data.errors?.length) throw new Error('Suggestions unavailable; continue typing.');
    result.items=(data.data?.repository?.mentionableUsers?.nodes||[]).filter(u=>/^[\w-]+$/.test(u.login)).map(u=>({text:'@'+u.login,label:u.name||u.login,kind:'user'}));
  } else {
    const root=`/repos/${owner}/${repo}`;
    let items;
    if(/^\d+$/.test(query)) {items=[await api(`${root}/issues/${query}`)];}
    else if(!query) {items=await api(`${root}/issues?state=all&sort=updated&direction=desc&per_page=15`);}
    else {
      // Strip search qualifiers from the free text so it cannot escape this repository.
      const words=query.match(/[\p{L}\p{N}_-]+/gu)||[];
      const params=new URLSearchParams({q:`repo:${owner}/${repo} in:title ${words.map(w=>'"'+w+'"').join(' ')}`,per_page:'15',sort:'updated'});
      items=(await api('/search/issues?'+params)).items;
    }
    result.items=(Array.isArray(items)?items:[]).filter(i=>Number.isSafeInteger(i.number)&&i.number>0).map(i=>({text:'#'+i.number,label:String(i.title||''),kind:i.pull_request?'PR':'issue'}));
  }
  return result;
}
