import type { FetchSettings, Override, Repo } from './api';

type FetchRepo = Pick<Repo, 'id' | 'pinned'> & {fetch_mode?: Override};

export function repoFetchEnabled(repo: FetchRepo, scope: FetchSettings['scope']) {
  return repo.fetch_mode === 'on' ||
    (repo.fetch_mode !== 'off' && (scope === 'all' || (scope === 'pinned' && repo.pinned)));
}

export async function launchFetch(
  repositories: FetchRepo[],
  scope: FetchSettings['scope'],
  fetch: (id: number) => Promise<unknown>,
) {
  const queue = repositories.filter(repo => repoFetchEnabled(repo, scope));
  const failed: number[] = [];
  let next = 0;
  async function worker() {
    while (next < queue.length) {
      const {id} = queue[next++];
      try { await fetch(id); }
      catch { failed.push(id); }
    }
  }
  await Promise.all([worker(), worker()]);
  return failed;
}
