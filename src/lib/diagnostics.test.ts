import { beforeEach, describe, expect, it, vi } from 'vitest';
const { nativeInvoke } = vi.hoisted(()=>({ nativeInvoke:vi.fn() }));
vi.mock('@tauri-apps/api/core',()=>({ invoke:nativeInvoke }));
import { captureLog, clearLogs, filterLogs, flushLogs, formatLogs, invoke, readLogs, type LogEntry } from './diagnostics';

const snapshot={entries:[],cursor:0,capacity:2000,log_file:null,persistence_error:null};
beforeEach(async()=>{nativeInvoke.mockResolvedValue(snapshot);await clearLogs();nativeInvoke.mockClear();});

describe('diagnostic transport',()=>{
  it('records command failures with context while omitting arguments and results',async()=>{
    nativeInvoke.mockImplementation((command:string)=>command==='connect_account'?Promise.reject(new Error('Connection unavailable')):Promise.resolve(snapshot));
    await expect(invoke('connect_account',{token:'argument-secret'})).rejects.toThrow('Connection unavailable');
    await flushLogs();
    const writes=nativeInvoke.mock.calls.filter(([command])=>command==='diagnostics_write');
    const text=JSON.stringify(writes);
    expect(text).toContain('connect_account');
    expect(text).toContain('Connection unavailable');
    expect(text).not.toContain('argument-secret');
  });

  it('uses the cursor without logging the log polling itself',async()=>{
    await readLogs(42);
    expect(nativeInvoke.mock.calls).toEqual([['diagnostics_snapshot',{after:42}]]);
  });

  it('retries a failed log transport without losing the original error',async()=>{
    captureLog('error','git','clone failed during a connection outage');
    nativeInvoke.mockRejectedValueOnce(new Error('Transport offline'));
    await flushLogs();
    nativeInvoke.mockResolvedValue(snapshot);
    await flushLogs();
    const writes=nativeInvoke.mock.calls.filter(([command])=>command==='diagnostics_write');
    expect(writes).toHaveLength(2);
    expect(writes[1][1].entries[0].message).toContain('clone failed during a connection outage');
  });

  it('bounds bursts and omits oversized messages before they can be cut through a credential',async()=>{
    for(let i=0;i<1005;i++)captureLog('debug','fixture',String(i));
    await flushLogs();
    const batch=nativeInvoke.mock.calls.find(([command])=>command==='diagnostics_write')?.[1].entries;
    expect(batch).toHaveLength(100);
    expect(batch[0].message).toContain('Dropped 5');
    await clearLogs();nativeInvoke.mockClear();
    captureLog('error','fixture','private-value'.repeat(10000));
    await flushLogs();
    const message=nativeInvoke.mock.calls[0][1].entries[0].message;
    expect(message).toContain('omitted');
    expect(message).not.toContain('private-value');
    nativeInvoke.mockClear();
    captureLog('error','fixture','🚀'.repeat(10000));
    await flushLogs();
    expect(nativeInvoke.mock.calls[0][1].entries[0].message).toContain('omitted');
  });
});

describe('log filtering and export text',()=>{
  const entries:LogEntry[]=[
    {id:1,timestamp:'2026-11-07T10:00:00Z',level:'info',source:'app',message:'Ready'},
    {id:2,timestamp:'2026-11-07T10:00:01Z',level:'error',source:'git',message:'clone failed\nPermission denied'},
  ];
  it('combines severity with case-insensitive source and message search',()=>{
    expect(filterLogs(entries,'error','PERMISSION').map(e=>e.id)).toEqual([2]);
    expect(filterLogs(entries,'all','GIT').map(e=>e.id)).toEqual([2]);
    expect(filterLogs(entries,'info','clone')).toHaveLength(0);
  });
  it('keeps timestamp, severity, source, and multiline diagnostics in copied text',()=>{
    expect(formatLogs(entries)).toContain('2026-11-07T10:00:01Z ERROR [git] clone failed\nPermission denied');
  });
});
