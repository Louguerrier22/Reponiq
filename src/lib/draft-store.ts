import {api, type Draft, type DraftScope, type Target} from './api';

export type DraftContext = {target: Target; scope: DraftScope};
const entries = new Map<string,Draft|null>();
const writes = new Map<string,Promise<void>>();
type PendingDraft = { context:DraftContext; value:Draft|null; failure?:{ reason:unknown } };
const unsaved = new Map<string,PendingDraft>();
export const draftKey = ({target,scope}:DraftContext) =>
  JSON.stringify([target.id,target.path,scope.branch,scope.account,scope.base,target.registration??null]);

export async function readDraft(context:DraftContext, persistent:boolean) {
  const key = draftKey(context);
  if (entries.has(key)) return entries.get(key) ?? null;
  const value = persistent ? await api.readDraft(context.target,context.scope) : null;
  if (value && !entries.has(key)) entries.set(key,value);
  return entries.has(key) ? entries.get(key) ?? null : value;
}

export function saveDraft(context:DraftContext, value:Draft|null, persistent:boolean) {
  const key = draftKey(context);
  entries.set(key,value);
  if (!persistent) {
    unsaved.delete(key);
    return Promise.resolve();
  }
  const pending: PendingDraft = { context, value };
  unsaved.set(key,pending);
  const previous = writes.get(key);
  // Only one write at a time per context. A superseded queued edit can be skipped.
  const next = (previous ?? Promise.resolve()).catch(()=>{}).then(async()=>{
    if (unsaved.get(key)!==pending) return;
    try {
      await api.saveDraft(context.target,context.scope,value);
    } catch (reason) {
      pending.failure = { reason };
      throw reason;
    }
    if (unsaved.get(key)===pending) unsaved.delete(key);
  });
  writes.set(key,next);
  return next.finally(()=>{if(writes.get(key)===next)writes.delete(key);});
}

export function forgetRepositoryDrafts(target:Target) {
  for (const key of entries.keys()) {
    const [id,path] = JSON.parse(key) as [number,string];
    if (id===target.id && path===target.path) {
      entries.delete(key);
      unsaved.delete(key);
    }
  }
}

export async function flushDraftWrites() {
  const failed = [...unsaved].filter(([key]) => !writes.has(key));
  for (const [, { context, value }] of failed) void saveDraft(context,value,true).catch(()=>{});
  while (writes.size) await Promise.allSettled(writes.values());
  for (const { failure } of unsaved.values()) {
    if (failure) throw failure.reason;
  }
}
