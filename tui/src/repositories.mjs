// Repository discovery is read-only. Only repositories with the viewer's open PRs
// are counted; a separate search can find any repository the token can access.
export async function discoverRepositories(input, token, fetcher = fetch) {
  const headers = { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2026-03-10', 'User-Agent': 'lazyreview-tui', ...(token ? { Authorization: `Bearer ${token}` } : {}) };
  async function api(path, payload) {
    let res;
    try { res = await fetcher('https://api.github.com' + path, { method: payload ? 'POST' : 'GET', headers: { ...headers, ...(payload ? { 'Content-Type': 'application/json' } : {}) }, ...(payload ? { body: JSON.stringify(payload) } : {}), signal: AbortSignal.timeout(25000) }); }
    catch { throw new Error('GitHub did not respond. Press r to retry.'); }
    if (!res.ok) throw new Error(res.status === 401 ? 'Sign in with gh auth login, then press r.' : `GitHub repository lookup failed (HTTP ${res.status}). Check access or retry after the rate limit resets.`);
    return res.json();
  }
  const query = String(input.query || '').trim();
  const page = input.page ?? 1;
  if (!Number.isInteger(page) || page < 1 || page > 34) throw new Error('Invalid repository result page.');
  if (query.length > 200) throw new Error('Repository search is too long.');
  if (query) {
    // Exact owner/repo lookup also works when a private repository is not searchable.
    if (/^[\w.-]+\/[\w.-]+$/.test(query)) {
      const repo = await api('/repos/' + query);
      if (!/^[\w.-]+\/[\w.-]+$/.test(repo.full_name || '')) throw new Error('GitHub returned an invalid repository.');
      return { repositories: [{ name: repo.full_name, description: String(repo.description || '') }], query, page: 1, hasNext: false, total: 1 };
    }
    const params = new URLSearchParams({ q: query, per_page: '30', page: String(page), sort: 'updated', order: 'desc' });
    const data = await api('/search/repositories?' + params);
    if (!Array.isArray(data.items)) throw new Error('GitHub returned an unexpected repository list.');
    const repositories = data.items.map(repo => {
      if (!/^[\w.-]+\/[\w.-]+$/.test(repo.full_name || '')) throw new Error('GitHub returned an invalid repository.');
      return { name: repo.full_name, description: String(repo.description || '') };
    });
    return { repositories, query, page, total: data.total_count, hasNext: page < 34 && page * 30 < Math.min(data.total_count, 1000), incomplete: !!data.incomplete_results, capped: data.total_count > 1000 };
  }
  if (!token) throw new Error('Run gh auth login, then r. Or press / to search a public repository, o for a PR URL, d for the demo.');
  const repos = new Map();
  let viewer, cursor = null;
  do {
    const response = await api('/graphql', { query: 'query($after:String) { viewer { id login pullRequests(first:100, states:OPEN, after:$after) { nodes { createdAt repository { nameWithOwner } } pageInfo { hasNextPage endCursor } } } }', variables: { after: cursor } });
    if (response.errors?.length || !response.data?.viewer?.pullRequests) throw new Error('GitHub could not list your open PR repositories. Check token access and press r.');
    viewer = response.data.viewer;
    for (const pr of viewer.pullRequests.nodes) {
      const name = pr.repository?.nameWithOwner;
      if (!/^[\w.-]+\/[\w.-]+$/.test(name || '')) throw new Error('GitHub returned an invalid repository.');
      repos.set(name, { name, openPrs: (repos.get(name)?.openPrs || 0) + 1, commits: null, lastContribution: [repos.get(name)?.lastContribution || '', pr.createdAt || ''].sort().at(-1) });
    }
    const info = viewer.pullRequests.pageInfo;
    if (!info.hasNextPage) break;
    if (!info.endCursor || info.endCursor === cursor) throw new Error('GitHub pagination stopped unexpectedly. Press r.');
    cursor = info.endCursor;
  } while (true);
  const repositories = [...repos.values()];
  let unavailable = false;
  for (let start = 0; start < repositories.length; start += 10) {
    const batch = repositories.slice(start, start + 10);
    const fields = batch.map((r, i) => {
      const [owner, name] = r.name.split('/');
      return `r${i}: repository(owner:${JSON.stringify(owner)}, name:${JSON.stringify(name)}) { defaultBranchRef { name target { ... on Commit { history(first:1, author:{id:$author}) { totalCount nodes { committedDate } } } } } }`;
    }).join('\n');
    try {
      const result = await api('/graphql', { query: `query($author:ID!) { ${fields} }`, variables: { author: viewer.id } });
      for (const [i, repo] of batch.entries()) {
        const value = result.data?.[`r${i}`];
        const failed = result.errors?.some(e => !e.path || e.path[0] === `r${i}`);
        const ref = value?.defaultBranchRef;
        const count = ref?.target?.history?.totalCount;
        if (!failed && value && (ref === null || Number.isInteger(count))) {
          repo.commits = ref === null ? 0 : count;
          repo.branch = ref?.name || '';
          const date = ref?.target?.history?.nodes?.[0]?.committedDate;
          if (typeof date === 'string' && date > repo.lastContribution) repo.lastContribution = date;
        } else unavailable = true;
      }
    } catch { unavailable = true; }
  }
  repositories.sort((a, b) => b.lastContribution.localeCompare(a.lastContribution) || (b.commits ?? -1) - (a.commits ?? -1) || a.name.localeCompare(b.name));
  return { repositories, viewer: viewer.login, query: '', total: repositories.length, page: 1, hasNext: false, warning: unavailable ? 'Some contribution dates and commit counts are unavailable. Press r to retry.' : '', countBasis: 'Your commits on the default branch' };
}

// Personal history deliberately uses connections, not Search's first-1000 results.
// Keep complete authored PR history separate from GitHub's recent-contribution list.
export async function discoverHistory(input, token, fetcher = fetch) {
  const repository = String(input.repository || '').trim();
  if (repository && !/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error('Enter a repository as owner/repo.');
  if (!repository && !token) throw new Error('Sign in with gh auth login for your history, or enter owner/repo to browse a public repository.');
  const headers = { Accept:'application/vnd.github+json', 'X-GitHub-Api-Version':'2026-03-10', 'User-Agent':'lazyreview-tui', ...(token ? {Authorization:`Bearer ${token}`} : {}) };
  async function api(path, body) {
    let response;
    try { response = await fetcher('https://api.github.com' + path, {method:body?'POST':'GET',headers:{...headers,...(body?{'Content-Type':'application/json'}:{})},...(body?{body:JSON.stringify(body)}:{}),signal:AbortSignal.timeout(25000)}); }
    catch { throw new Error('GitHub did not respond. Close Search and reopen to retry.'); }
    if (!response.ok) throw new Error(response.status===401?'Sign in with gh auth login, then retry.':`GitHub history lookup failed (HTTP ${response.status}). Check repository access or the rate limit.`);
    const data=await response.json();
    if(data.errors?.length) throw new Error('GitHub could not complete this history lookup. Check token access and retry.');
    return data;
  }
  const validName=name=>typeof name==='string'&&/^[\w.-]+\/[\w.-]+$/.test(name);
  function item(node, rest=false) {
    const raw=rest?node.html_url:node.url;
    let url;try {url=new URL(raw);}catch {throw new Error('GitHub returned an invalid PR URL.');}
    const match=url.pathname.match(/^\/([\w.-]+)\/([\w.-]+)\/pull\/([1-9]\d*)$/);
    if(url.origin!=='https://github.com'||url.username||url.password||url.search||url.hash||!match) throw new Error('GitHub returned an unsupported PR URL.');
    return {url:url.href,repository:`${match[1]}/${match[2]}`,number:Number(match[3]),title:String(node.title||''),author:String((rest?node.user:node.author)?.login||'ghost'),created:String((rest?node.created_at:node.createdAt)||''),updated:String((rest?node.updated_at:node.updatedAt)||''),draft:!!(rest?node.draft:node.isDraft),state:rest?(node.merged_at?'MERGED':String(node.state||'').toUpperCase()):String(node.state||'')};
  }
  if(repository) {
    const items=new Map();let page=1;
    while(true) {
      const params=new URLSearchParams({state:'all',sort:'created',direction:'desc',per_page:'100',page:String(page++)});
      const nodes=await api(`/repos/${repository}/pulls?${params}`);
      if(!Array.isArray(nodes)) throw new Error('GitHub returned an unexpected PR list.');
      let added=0;
      for(const node of nodes) {const pr=item(node,true);if(!items.has(pr.url))added++;items.set(pr.url,pr);}
      if(nodes.length<100)break;
      if(!added) throw new Error('GitHub pagination stopped unexpectedly; retry the repository lookup.');
    }
    return {kind:'pulls',repository,items:[...items.values()].sort((a,b)=>b.created.localeCompare(a.created)||b.number-a.number),warning:''};
  }
  let viewer,after=null;const items=new Map(),repos=new Map(),seen=new Set();
  const addRepo=(name,description='')=>{if(!validName(name)) throw new Error('GitHub returned an invalid repository.');if(!repos.has(name))repos.set(name,{name,description:String(description||''),prs:0,lastContribution:''});return repos.get(name);};
  function next(connection) {
    if(!connection?.pageInfo||!Array.isArray(connection.nodes))throw new Error('GitHub returned an unexpected history page.');
    if(!connection.pageInfo.hasNextPage)return null;
    const cursor=connection.pageInfo.endCursor;
    if(!cursor||seen.has(cursor))throw new Error('GitHub pagination stopped unexpectedly. Close Search and retry.');
    seen.add(cursor);return cursor;
  }
  do {
    const response=await api('/graphql',{query:'query($after:String) { viewer { id login pullRequests(first:100, after:$after, orderBy:{field:CREATED_AT,direction:DESC}) { nodes { url number title createdAt updatedAt state isDraft author { login } repository { nameWithOwner } } pageInfo { hasNextPage endCursor } } } }',variables:{after}});
    viewer=response.data?.viewer;
    if(!viewer?.id||!viewer.login||!viewer.pullRequests)throw new Error('GitHub could not load your PR history.');
    after=next(viewer.pullRequests);
    for(const node of viewer.pullRequests.nodes) {
      if(!node)continue;const pr=item(node);if(items.has(pr.url))continue;items.set(pr.url,pr);
      const repo=addRepo(pr.repository);repo.prs++;repo.lastContribution=[repo.lastContribution,pr.created].sort().at(-1);
    }
  }while(after);
  const warnings=[];
  after=null;seen.clear();
  try {
    do {
      const response=await api('/graphql',{query:'query($after:String) { viewer { repositoriesContributedTo(first:100, after:$after, includeUserRepositories:true, contributionTypes:[COMMIT,ISSUE,PULL_REQUEST,REPOSITORY], orderBy:{field:UPDATED_AT,direction:DESC}) { nodes { nameWithOwner description } pageInfo { hasNextPage endCursor } } } }',variables:{after}});
      const connection=response.data?.viewer?.repositoriesContributedTo;after=next(connection);
      for(const repo of connection.nodes)if(repo)addRepo(repo.nameWithOwner,repo.description);
    }while(after);
  }catch {warnings.push('Recent contribution repositories are partly unavailable; your authored PR history is complete.');}
  const repositories=[...repos.values()];
  // GitHub sorts repositories by repository activity, not the viewer's activity.
  // Compare the viewer's newest default-branch commit to their newest authored PR.
  let datesUnavailable=false;
  for(let start=0;start<repositories.length;start+=10) {
    const batch=repositories.slice(start,start+10);
    const fields=batch.map((repo,i)=>{const[owner,name]=repo.name.split('/');return `r${i}:repository(owner:${JSON.stringify(owner)},name:${JSON.stringify(name)}) { defaultBranchRef { target { ... on Commit { history(first:1,author:{id:$author}) { nodes { committedDate } } } } } }`;}).join('\n');
    try {
      const response=await api('/graphql',{query:`query($author:ID!){${fields}}`,variables:{author:viewer.id}});
      for(const[i,repo]of batch.entries()) {
        const value=response.data?.[`r${i}`];if(!value){datesUnavailable=true;continue;}
        const date=value.defaultBranchRef?.target?.history?.nodes?.[0]?.committedDate;
        if(typeof date==='string'&&date>repo.lastContribution)repo.lastContribution=date;
      }
    }catch {datesUnavailable=true;}
  }
  if(datesUnavailable)warnings.push('Some contribution dates are unavailable.');
  repositories.sort((a,b)=>b.lastContribution.localeCompare(a.lastContribution)||a.name.localeCompare(b.name));
  return {kind:'catalog',viewer:viewer.login,repositories,items:[...items.values()].sort((a,b)=>b.created.localeCompare(a.created)||b.number-a.number),warning:warnings.join(' '),orderBasis:'Newest authored PR or default-branch commit'};
}
