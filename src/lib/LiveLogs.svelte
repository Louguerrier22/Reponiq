<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Copy, Download, LoaderCircle, Pause, Play, Search, Trash2, X } from 'lucide-svelte';
  import Modal from './Modal.svelte';
  import { clearLogs, exportLogs, filterLogs, formatLogs, readLogs, type LogEntry, type LogLevel } from './diagnostics';
  let {onclose}:{onclose:()=>void}=$props();
  let entries=$state<LogEntry[]>([]),level=$state<LogLevel|'all'>('all'),query=$state('');
  let paused=$state(false),follow=$state(true),loading=$state(true),exporting=$state(false),clearing=$state(false);
  let error=$state(''),readError=$state(''),notice=$state(''),file=$state<string|null>(null),persistenceError=$state<string|null>(null),capacity=$state(2000);
  let viewport:HTMLDivElement;
  let cursor=0,fetching=false,alive=true,generation=0;
  const visible=$derived(filterLogs(entries,level,query));
  const errors=$derived(entries.filter(e=>e.level==='error').length);
  async function poll(){
    if(fetching||clearing)return;
    fetching=true;const request=generation;
    try{
      const result=await readLogs(cursor);
      if(!alive||request!==generation||paused)return;
      if(!result||!Array.isArray(result.entries))throw new Error('The logging service returned an unexpected response.');
      cursor=result.cursor;capacity=result.capacity;file=result.log_file;persistenceError=result.persistence_error;
      if(result.entries.length){entries=[...entries,...result.entries].slice(-capacity);if(follow){await tick();if(alive&&viewport)viewport.scrollTop=viewport.scrollHeight;}}
      readError='';
    }catch(e){if(alive&&request===generation)readError=String(e);}
    finally{fetching=false;if(alive)loading=false;}
  }
  onMount(()=>{void poll();const timer=setInterval(()=>{if(!paused)void poll();},750);return()=>{alive=false;generation++;clearInterval(timer);};});
  async function togglePause(){paused=!paused;if(!paused)await poll();}
  async function copy(){error='';try{await navigator.clipboard.writeText(formatLogs(visible));notice=`Copied ${visible.length} log entries.`;}catch{error='Could not copy logs. Use Download logs to save them instead.';}}
  async function download(){if(exporting)return;exporting=true;error='';try{const path=await exportLogs(visible);if(alive&&path)notice=`Saved logs to ${path}`;}catch(e){if(alive)error=String(e);}finally{exporting=false;}}
  async function clear(){if(clearing)return;clearing=true;generation++;error='';try{const result=await clearLogs();if(alive){entries=[];cursor=result.cursor;persistenceError=result.persistence_error;notice='Log history cleared.';}}catch(e){if(alive)error=String(e);}finally{clearing=false;}}
  async function followLatest(){follow=!follow;if(follow){await tick();if(viewport)viewport.scrollTop=viewport.scrollHeight;}}
  function time(timestamp:string){const date=new Date(timestamp);return Number.isNaN(date.getTime())?timestamp:date.toLocaleTimeString(undefined,{hour12:false,hour:'2-digit',minute:'2-digit',second:'2-digit'});}
</script>

<Modal label="Live logs" canClose={!exporting} {onclose}>
  <section class="live-logs">
    <div class="modal-top"><div><h2>Live logs</h2><p>Runtime activity and error details for troubleshooting.</p></div><button class="top-icon" aria-label="Close live logs" disabled={exporting} onclick={onclose}><X size={18}/></button></div>
    <div class="log-summary"><span class:paused><i></i>{paused?'Paused':'Live'}</span><span>{entries.length} retained · {errors} {errors===1?'error':'errors'}</span><small>Latest {capacity.toLocaleString()} entries</small></div>
    <div class="log-toolbar">
      <div class="search-input"><Search size={15}/><input aria-label="Search logs" placeholder="Search messages or sources" bind:value={query}/></div>
      <label class="log-level">Level<select aria-label="Log level" bind:value={level}><option value="all">All levels</option><option value="error">Errors</option><option value="warn">Warnings</option><option value="info">Info</option><option value="debug">Debug</option></select></label>
      <button class="secondary" onclick={togglePause} aria-pressed={paused}>{#if paused}<Play size={14}/> Resume{:else}<Pause size={14}/> Pause{/if}</button>
    </div>
    {#if error}<p role="alert" class="connection-error">{error}</p>{/if}
    {#if readError}<p role="alert" class="connection-error">{readError} <button class="text-action" onclick={()=>{paused=false;void poll();}}>Retry</button></p>{/if}
    {#if persistenceError}<p role="alert" class="connection-error">{persistenceError} Live entries are still available.</p>{/if}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (This scrollable region must be reachable for keyboard scrolling.) -->
    <div class="log-viewport" bind:this={viewport} role="region" aria-label="Log entries" tabindex="0" onscroll={()=>{follow=viewport.scrollHeight-viewport.scrollTop-viewport.clientHeight<30;}}>
      {#if loading}<div class="log-empty" role="status"><LoaderCircle size={18} class="spin"/> Loading logs…</div>
      {:else if !visible.length}<div class="log-empty">{entries.length?'No logs match these filters.':'No log entries yet. Activity and errors will appear here.'}</div>
      {:else}{#each visible as entry (entry.id)}<article class="log-entry" class:log-error={entry.level==='error'} class:log-warning={entry.level==='warn'}><div class="log-entry-meta"><time datetime={entry.timestamp} title={entry.timestamp}>{time(entry.timestamp)}</time><span class={'log-badge log-'+entry.level}>{entry.level}</span><span class="log-source" title={entry.source}>{entry.source}</span></div><pre>{entry.message}</pre></article>{/each}{/if}
    </div>
    <div class="log-footer"><button class="text-action" aria-pressed={follow} onclick={followLatest}>{follow?'Following latest':'Follow latest'}</button><span>{visible.length} shown</span><div><button class="secondary" disabled={!visible.length||clearing} onclick={copy}><Copy size={14}/> Copy logs</button><button class="secondary" disabled={!visible.length||exporting||clearing} onclick={download}>{#if exporting}<LoaderCircle size={14} class="spin"/>{:else}<Download size={14}/>{/if} Download logs</button><button class="secondary" disabled={clearing||exporting||!entries.length} onclick={clear}><Trash2 size={14}/> Clear logs</button></div></div>
    {#if notice}<p role="status" class="log-notice">{notice}</p>{/if}
    <p class="log-privacy">Logs stay on this device. Review local paths and private details before sharing.</p>
    {#if file}<p class="log-file">Saved locally: <code>{file}</code></p>{/if}
  </section>
</Modal>

<style>
  :global(dialog.modal:has(.live-logs)){width:min(1100px,calc(100vw - 32px));max-width:1100px}
  .live-logs{min-width:0}.modal-top p{font-size:13px;line-height:1.6;margin:7px 0 0}.log-summary{display:flex;align-items:center;gap:18px;flex-wrap:wrap;margin:20px 0;color:#c5bbd1;font-size:12px}.log-summary>span:first-child{display:flex;align-items:center;gap:7px;color:#acdcbc}.log-summary i{width:7px;height:7px;border-radius:50%;background:#83cb9d}.log-summary .paused{color:#e1c58e}.log-summary .paused i{background:#dfbd79}.log-summary small{margin-left:auto}
  .log-toolbar{display:flex;align-items:end;gap:12px;margin-bottom:16px}.log-toolbar .search-input{flex:1;min-width:120px;height:43px}.log-toolbar .search-input input{background:transparent;box-shadow:none;border:0;color:inherit;min-width:0;padding:0;height:auto}.log-level{display:flex;flex-direction:column;gap:5px;margin:0!important;font-size:11px!important}.log-level select{min-width:130px}.log-toolbar button{min-height:43px}.log-viewport{height:min(49vh,520px);min-height:220px;overflow:auto;overscroll-behavior:contain;scrollbar-width:thin;border:1px solid #554c60;border-radius:10px;background:#1a1921;outline-offset:3px}.log-entry{border-bottom:1px solid #332e3d;padding:12px 15px}.log-entry:last-child{border-bottom:0}.log-error{background:#42252d40;border-left:3px solid #e88b9e}.log-warning{background:#443b2440;border-left:3px solid #d8b576}.log-entry-meta{display:flex;align-items:center;gap:12px;color:#a99eb8;font-size:11px;min-width:0}.log-entry time{font-variant-numeric:tabular-nums}.log-badge{text-transform:uppercase;font-size:9px;font-weight:700;letter-spacing:.05em;border-radius:4px;padding:3px 6px;background:#343040;color:#c8bdd9}.log-badge.log-error{color:#ffc1cc;background:#633341;border:0}.log-badge.log-warn{color:#edcf9b;background:#51442b}.log-badge.log-info{color:#b8d4f8;background:#2c3c56}.log-source{overflow-wrap:anywhere;min-width:0}.log-entry pre{font-family:'Cascadia Code',Consolas,ui-monospace,monospace;font-size:12px;line-height:1.7;white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;color:#e5ddec;margin:8px 0 0}.log-empty{display:flex;align-items:center;justify-content:center;gap:10px;padding:45px 20px;color:#bcaecc;font-size:13px;text-align:center}
  .log-footer,.log-footer>div{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.log-footer{justify-content:space-between;margin-top:16px}.log-footer>span{font-size:11px;color:#baacc8}.log-footer button{font-size:11px;min-height:36px}.log-notice{font-size:12px;color:#acdcbc;overflow-wrap:anywhere}.log-privacy{font-size:12px;color:#c5b8d0;margin:18px 0 0;line-height:1.6}.log-file{font-size:11px;color:#a99ab8;line-height:1.6;margin:6px 0 0;overflow-wrap:anywhere}.log-file code{font-size:11px}
  @media(max-width:650px){.log-toolbar{flex-wrap:wrap}.log-toolbar .search-input{flex-basis:100%}.log-level{flex:1}.log-summary small{margin-left:0}.log-footer>div{width:100%}.log-footer>div button{flex:1;padding:8px}.log-entry{padding:11px 10px}.log-entry-meta{gap:8px;font-size:10px}.log-entry pre{font-size:11px}.log-viewport{height:40vh}}
</style>
