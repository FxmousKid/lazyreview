// Read-only chooser API. Kept separate from review mutations and injectable for tests.
export async function discoverPullRequests(input, token, fetcher = fetch) {
  const scope = input.scope || 'involved';
  if (!['involved', 'requested', 'authored', 'repository'].includes(scope)) throw new Error('Unknown PR list.');
  const page = input.page ?? 1;
  if (!Number.isInteger(page) || page < 1 || page > 20) throw new Error('Choose a result page between 1 and 20.');
  const repo = String(input.repository || '').trim();
  if (scope === 'repository' && !/^[\w.-]+\/[\w.-]+$/.test(repo)) throw new Error('Enter a repository as owner/repo.');
  if (!token && scope !== 'repository') throw new Error('Sign in with gh auth login, then press r. Or press 4 to browse a public repository, o to paste a PR URL, or d for the demo.');
  async function get(path) {
    let response;
    try {
      response = await fetcher('https://api.github.com' + path, {
        method: 'GET',
        headers: { Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2026-03-10', 'User-Agent': 'lazyreview-tui', ...(token ? { Authorization: `Bearer ${token}` } : {}) },
        signal: AbortSignal.timeout(25000),
      });
    } catch { throw new Error('GitHub did not respond. Press r to retry.'); }
    if (!response.ok) {
      const message = response.status === 401 ? 'GitHub authentication expired. Run gh auth login, then press r.'
        : response.status === 403 || response.status === 429 ? 'GitHub denied the search. Check repository access or wait for the search rate limit, then press r.'
        : response.status === 404 || response.status === 422 ? 'Repository search is unavailable. Check owner/repo and your GitHub access.'
        : `GitHub returned HTTP ${response.status}. Press r to retry.`;
      throw new Error(message);
    }
    return response.json();
  }
  let viewer = null;
  if (scope !== 'repository') {
    const user = await get('/user');
    if (typeof user.login !== 'string' || !/^[\w-]+$/.test(user.login)) throw new Error('GitHub did not return a valid signed-in user.');
    viewer = user.login;
  }
  const qualifier = scope === 'repository' ? `repo:${repo}` : `${scope === 'requested' ? 'review-requested' : scope === 'authored' ? 'author' : 'involves'}:${viewer}`;
  const query = `is:pr is:open ${qualifier}`;
  const params = new URLSearchParams({ q: query, sort: 'updated', order: 'desc', per_page: '50', page: String(page) });
  const data = await get('/search/issues?' + params);
  if (!Array.isArray(data.items) || !Number.isInteger(data.total_count) || data.total_count < 0) throw new Error('GitHub returned an unexpected PR list. Press r to retry.');
  const items = data.items.filter(item => item.pull_request).map(item => {
    let url;
    try { url = new URL(item.html_url); } catch { throw new Error('GitHub returned an invalid pull request URL.'); }
    const match = url.pathname.match(/^\/([\w.-]+)\/([\w.-]+)\/pull\/([1-9]\d*)$/);
    if (url.origin !== 'https://github.com' || url.username || url.password || url.search || url.hash || !match) throw new Error('GitHub returned an unsupported pull request URL.');
    return { url: url.href, repository: `${match[1]}/${match[2]}`, number: Number(match[3]), title: String(item.title || ''), author: String(item.user?.login || 'ghost'), updated: String(item.updated_at || ''), draft: !!item.draft, comments: Number(item.comments || 0) };
  });
  return { items, viewer, scope, repository: repo, page, total: data.total_count, incomplete: !!data.incomplete_results, capped: data.total_count > 1000, hasNext: page < 20 && page * 50 < data.total_count && data.items.length === 50 };
}
