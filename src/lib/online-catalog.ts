import type { OnlineRepo, Page } from "./api";

export async function loadOnlineCatalog(
  fetchPage: (page: number) => Promise<Page<OnlineRepo>>,
  isCurrent: () => boolean,
): Promise<OnlineRepo[] | null> {
  const repositories = new Map<number, OnlineRepo>();
  // This matches the native limit for catalog requests.
  for (let page = 1; page <= 400 && isCurrent(); page++) {
    const result = await fetchPage(page);
    if (!isCurrent()) return null;
    for (const repo of result.items) repositories.set(repo.id, repo);
    if (!result.has_more) return [...repositories.values()];
  }
  if (!isCurrent()) return null;
  throw new Error("This account’s catalog is too large to search completely.");
}
