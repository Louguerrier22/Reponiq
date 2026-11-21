import { expect, test } from 'vitest';
import { launchFetch, repoFetchEnabled } from './launch-fetch';

test('selected scope fetches only repositories explicitly enabled', () => {
  expect(repoFetchEnabled({ id: 1, pinned: true }, 'selected')).toBe(false);
  expect(repoFetchEnabled({ id: 2, pinned: false, fetch_mode: 'on' }, 'selected')).toBe(true);
  expect(repoFetchEnabled({ id: 3, pinned: true, fetch_mode: 'off' }, 'selected')).toBe(false);
});

test('pinned scope fetches only pins, with at most two in flight, and keeps failures retryable', async () => {
  let active = 0;
  let peak = 0;
  const visited: number[] = [];
  const failed = await launchFetch([
    {id: 1, pinned: true}, {id: 2, pinned: true}, {id: 3, pinned: false}, {id: 4, pinned: true},
  ], 'pinned', async id => {
    visited.push(id);
    peak = Math.max(peak, ++active);
    await Promise.resolve();
    active--;
    if (id === 2) throw new Error('offline');
  });
  expect(visited).toEqual([1, 2, 4]);
  expect(peak).toBe(2);
  expect(failed).toEqual([2]);
});

test('explicit launch overrides take precedence over pins and the all-repositories scope', async () => {
  const repos = [{id:1,pinned:true,fetch_mode:'off' as const},{id:2,pinned:false,fetch_mode:'on' as const},{id:3,pinned:false}];
  const called:number[]=[];
  await launchFetch(repos,'pinned',async id=>{called.push(id);});
  expect(called).toEqual([2]);
  called.length=0;
  await launchFetch(repos,'all',async id=>{called.push(id);});
  expect(called).toEqual([2,3]);
});
