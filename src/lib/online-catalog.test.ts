import { expect, test, vi } from "vitest";
import type { OnlineRepo } from "./api";
import { loadOnlineCatalog } from "./online-catalog";

const repo = (id: number): OnlineRepo => ({
  id, name: `repo-${id}`, full_name: `owner/repo-${id}`, private: false,
});

test("loads beyond the first page and deduplicates repositories", async () => {
  const fetchPage = vi.fn()
    .mockResolvedValueOnce({ items: [repo(1)], has_more: true })
    .mockResolvedValueOnce({ items: [repo(1), repo(2)], has_more: false });
  await expect(loadOnlineCatalog(fetchPage, () => true)).resolves.toEqual([repo(1), repo(2)]);
  expect(fetchPage.mock.calls).toEqual([[1], [2]]);
});

test("switching accounts stops pagination after the outstanding request", async () => {
  let current = true;
  const fetchPage = vi.fn(async () => {
    current = false;
    return { items: [repo(1)], has_more: true };
  });
  await expect(loadOnlineCatalog(fetchPage, () => current)).resolves.toBeNull();
  expect(fetchPage).toHaveBeenCalledTimes(1);
});
