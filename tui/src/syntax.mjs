// Independent of the mutable review bridge: every read carries its own snapshot.
export async function syntaxSources(input, sample, readContent) {
  const file = input.file;
  const read = async side => {
    if ((side === 'LEFT' && file.status === 'added') || (side === 'RIGHT' && file.status === 'removed')) return '';
    if (input.demo) {
      if (input.expectedHead !== sample.head || input.expectedBase !== sample.base) return null;
      const fixture = sample.files.find(f => f.filename === file.filename);
      return (side === 'LEFT' ? fixture?.originalContent : fixture?.content) ?? null;
    }
    try {
      const result = await readContent({repository:side === 'LEFT' ? input.baseRepo : input.headRepo, commit:side === 'LEFT' ? input.expectedBase : input.expectedHead, path:side === 'LEFT' ? (file.previous_filename || file.filename) : file.filename, side});
      return result.commit === (side === 'LEFT' ? input.expectedBase : input.expectedHead) ? result.content : null;
    } catch { return null; } // Diff remains usable when a side is unavailable.
  };
  const [left, right] = await Promise.all([read('LEFT'), read('RIGHT')]);
  return {left, right, leftPath:file.previous_filename || file.filename, rightPath:file.filename};
}

// The PR loader has already captured these immutable repositories/commits/paths.
// This read does not re-fetch mutable PR metadata or alter the guarded write API.
export async function readSnapshotContent(input, token, fetcher = fetch) {
  const {repository,commit,path} = input;
  if (typeof repository !== 'string' || repository.length > 256 || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository) || repository.split('/').some(p => p === '.' || p === '..')) throw Error('Invalid repository');
  if (typeof commit !== 'string' || !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/i.test(commit)) throw Error('An immutable commit is required');
  if (typeof path !== 'string' || path.length > 4096 || /[\x00-\x1f\x7f]/.test(path) || path.split('/').some(p => !p || p === '.' || p === '..')) throw Error('Invalid file path');
  const endpoint = `https://api.github.com/repos/${repository}/contents/${path.split('/').map(encodeURIComponent).join('/')}?ref=${commit}`;
  const response = await fetcher(endpoint, {
    method:'GET', redirect:'error',
    headers:{Accept:'application/vnd.github+json','X-GitHub-Api-Version':'2026-03-10','User-Agent':'lazyreview-poc',...(token ? {Authorization:`Bearer ${token}`} : {})},
    signal:AbortSignal.timeout(10000),
  });
  if (!response.ok) throw Error('Syntax source unavailable');
  const data = await response.json();
  if (data.type !== 'file' || data.encoding !== 'base64' || typeof data.content !== 'string' || data.content.length > 280000 || !Number.isInteger(data.size) || data.size < 0 || data.size > 200000) throw Error('Unsupported syntax source');
  const bytes = Uint8Array.from(atob(data.content.replace(/\s/g,'')), c => c.charCodeAt(0));
  if (bytes.length > 200000 || bytes.length !== data.size) throw Error('Unsupported syntax source');
  const content = new TextDecoder('utf-8',{fatal:true}).decode(bytes);
  if (content.includes('\0')) throw Error('Binary syntax source');
  return {content,commit};
}
