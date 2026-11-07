import { invoke as nativeInvoke } from '@tauri-apps/api/core';

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';
export type LogEntry = { id:number; timestamp:string; level:LogLevel; source:string; message:string };
export type LogSnapshot = { entries:LogEntry[]; cursor:number; capacity:number; log_file:string|null; persistence_error:string|null };
type ClientEntry = Pick<LogEntry,'level'|'source'|'message'>;
const pending: ClientEntry[] = [];
let flushing: Promise<void>|null = null;
let dropped = 0;

function describe(value:unknown):string {
  if(value instanceof Error)return value.stack ?? `${value.name}: ${value.message}`;
  if(typeof value==='string')return value;
  try{return JSON.stringify(value, (key,item)=>/^(?:password|passwd|token|access_token|refresh_token|authorization|client_secret|device_code|user_code)$/i.test(key)?'[REDACTED]':item) ?? String(value);}catch{return '[Unserializable value]';}
}

export function captureLog(level:LogLevel,source:string,message:string) {
  if(pending.length>=1000){pending.shift();dropped++;}
  const oversized=message.length>32768||new TextEncoder().encode(message).byteLength>32768;
  pending.push({level,source:source.slice(0,120),message:oversized?`Log message omitted: exceeds the 32 KB limit (${message.length} characters).`:message});
}

export function flushLogs():Promise<void> {
  if(flushing)return flushing;
  if(!pending.length&&!dropped)return Promise.resolve();
  const entries=pending.splice(0,100);
  if(dropped){entries.splice(0,1,{level:'warn',source:'diagnostics',message:`Dropped ${dropped} log entries during a burst.`});dropped=0;}
  flushing=nativeInvoke<void>('diagnostics_write',{entries}).catch(()=>{
    // Retry transient transport failures without recursively logging them.
    pending.unshift(...entries);
    if(pending.length>1000){const lost=pending.length-1000;pending.splice(0,lost);dropped+=lost;}
  }).finally(()=>{flushing=null;});
  return flushing;
}

export async function invoke<T>(command:string,args?:Record<string,unknown>):Promise<T> {
  const start=performance.now();
  captureLog('debug','commands',`${command} started`);
  try {
    const result=await nativeInvoke<T>(command,args);
    captureLog('debug','commands',`${command} completed in ${Math.round(performance.now()-start)} ms`);
    return result;
  }catch(error){
    captureLog('error',`commands.${command}`,`${command} failed after ${Math.round(performance.now()-start)} ms\n${describe(error)}`);
    void flushLogs();
    throw error;
  }
}

export async function readLogs(after=0):Promise<LogSnapshot> {
  await flushLogs();
  return nativeInvoke<LogSnapshot>('diagnostics_snapshot',{after});
}

export async function clearLogs():Promise<LogSnapshot> {
  await flushLogs();
  pending.length=0;
  return nativeInvoke<LogSnapshot>('diagnostics_clear');
}

export function exportLogs(entries:LogEntry[]):Promise<string|null> {
  return nativeInvoke<string|null>('diagnostics_export',{text:formatLogs(entries)});
}

export function formatLogs(entries:LogEntry[]):string {
  return entries.map(entry=>`${entry.timestamp} ${entry.level.toUpperCase().padEnd(5)} [${entry.source}] ${entry.message}`).join('\n');
}

export function filterLogs(entries:LogEntry[],level:LogLevel|'all',query:string):LogEntry[] {
  const search=query.trim().toLocaleLowerCase();
  return entries.filter(entry=>(level==='all'||entry.level===level)&&(!search||`${entry.source} ${entry.message}`.toLocaleLowerCase().includes(search)));
}

export function startDiagnostics():()=>void {
  const originals=new Map<keyof Console,unknown>();
  for(const [method,level] of [['debug','debug'],['log','info'],['info','info'],['warn','warn'],['error','error']] as const){
    const original=console[method];
    originals.set(method,original);
    console[method]=(...values:unknown[])=>{
      original.apply(console,values);
      captureLog(level,'console',values.map(describe).join(' '));
    };
  }
  const error=(event:ErrorEvent)=>{captureLog('error','window',`${event.message}\n${event.filename}:${event.lineno}:${event.colno}\n${event.error?describe(event.error):''}`);void flushLogs();};
  const rejection=(event:PromiseRejectionEvent)=>{captureLog('error','promise',describe(event.reason));void flushLogs();};
  const hide=()=>{if(document.visibilityState==='hidden')void flushLogs();};
  window.addEventListener('error',error);
  window.addEventListener('unhandledrejection',rejection);
  document.addEventListener('visibilitychange',hide);
  const timer=setInterval(()=>void flushLogs(),500);
  captureLog('info','app','Frontend initialized');
  return()=>{
    clearInterval(timer);
    window.removeEventListener('error',error);
    window.removeEventListener('unhandledrejection',rejection);
    document.removeEventListener('visibilitychange',hide);
    for(const [method,original] of originals)Reflect.set(console,method,original);
    void flushLogs();
  };
}
