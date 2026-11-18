import type { FetchSettings, Override, Repo } from './api';

export async function launchFetch(
  repositories: (Pick<Repo, 'id' | 'pinned'> & {fetch_mode?:Override})[],
  scope: FetchSettings['scope'],
  fetch: (id: number) => Promise<unknown>,
) {
  const queue = repositories.filter(repo => repo.fetch_mode==='on'
    || (repo.fetch_mode!=='off' && (scope==='all' || repo.pinned)));
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
