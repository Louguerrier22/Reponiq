import { expect, test, vi } from 'vitest';
import { api } from './api';
import { flushDraftWrites, saveDraft, type DraftContext } from './draft-store';

function pendingWrite() {
  let resolve!: () => void;
  const promise = new Promise<void>(done => { resolve = done; });
  return { promise, resolve };
}

test('flush waits for the last edit queued while an earlier draft write is in flight', async () => {
  const context: DraftContext = {
    target: { id: 1, path: '/repo' },
    scope: { branch: 'main', account: '', base: '' },
  };
  const first = pendingWrite();
  const last = pendingWrite();
  const write = vi.spyOn(api, 'saveDraft')
    .mockImplementationOnce(() => first.promise)
    .mockImplementationOnce(() => last.promise);

  try {
    const earlier = saveDraft(context, { title: 'earlier', body: '' }, true);
    await vi.waitFor(() => expect(write).toHaveBeenCalledTimes(1));
    let flushed = false;
    const closing = flushDraftWrites().then(() => { flushed = true; });
    const latest = saveDraft(context, { title: 'latest', body: '' }, true);
    first.resolve();
    await vi.waitFor(() => expect(write).toHaveBeenCalledTimes(2));
    expect(flushed).toBe(false);
    last.resolve();
    await Promise.all([earlier, latest, closing]);
    expect(flushed).toBe(true);
  } finally {
    first.resolve();
    last.resolve();
    write.mockRestore();
  }
});

test('flush retries a draft write that failed before the user requested quit', async () => {
  const context: DraftContext = {
    target: { id: 2, path: '/other' },
    scope: { branch: 'main', account: '', base: '' },
  };
  const draft = { title: 'Keep this draft', body: '' };
  const write = vi.spyOn(api, 'saveDraft')
    .mockRejectedValueOnce(new Error('Workspace busy'))
    .mockResolvedValueOnce(undefined);
  try {
    await expect(saveDraft(context, draft, true)).rejects.toThrow('Workspace busy');
    await flushDraftWrites();
    expect(write).toHaveBeenCalledTimes(2);
    expect(write).toHaveBeenLastCalledWith(context.target, context.scope, draft);
  } finally {
    write.mockRestore();
  }
});

test('a failed superseded write does not cancel quit after the latest draft saves', async () => {
  const context: DraftContext = {
    target: { id: 3, path: '/third' },
    scope: { branch: 'main', account: '', base: '' },
  };
  let reject!: (reason: Error) => void;
  const pending = new Promise<void>((_, fail) => { reject = fail; });
  const write = vi.spyOn(api, 'saveDraft')
    .mockImplementationOnce(() => pending)
    .mockResolvedValueOnce(undefined);
  try {
    const earlier = saveDraft(context, { title: 'earlier', body: '' }, true).catch(() => {});
    await vi.waitFor(() => expect(write).toHaveBeenCalledTimes(1));
    const closing = flushDraftWrites();
    const latest = saveDraft(context, { title: 'latest', body: '' }, true);
    reject(new Error('Superseded write failed'));
    await expect(closing).resolves.toBeUndefined();
    await Promise.all([earlier, latest]);
    expect(write).toHaveBeenCalledTimes(2);
  } finally {
    write.mockRestore();
  }
});
