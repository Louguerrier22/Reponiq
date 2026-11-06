<script lang="ts">

  import { onMount, tick } from 'svelte';

  import Modal from './lib/Modal.svelte';
  import AddRepository from './lib/AddRepository.svelte';
  import WorkspaceFolder from './lib/WorkspaceFolder.svelte';
  import ImportRepositories from './lib/ImportRepositories.svelte';
  import Walkthrough from './lib/Walkthrough.svelte';
  import PlatformIcon from './lib/PlatformIcon.svelte';
  import { remoteInfo } from './lib/platform';
  import TokenInput from './lib/TokenInput.svelte';
  import CommitHistory from './lib/CommitHistory.svelte';
  import ConflictEditor from './lib/ConflictEditor.svelte';
  import DiffView from './lib/DiffView.svelte';
  import PullRequests from './lib/PullRequests.svelte';

  import { Activity, ArrowDown, ArrowLeftRight, ArrowUp, Check, CircleAlert, Clock3, Code2, FileCode, Folder, FolderGit2, GitBranch, GitCommitHorizontal, Github, History, LayoutDashboard, LoaderCircle, Pin, Plus, RefreshCw, Search, Settings, ShieldCheck, Tags, X } from 'lucide-svelte';

  import { api, filteredRepos, type Repo, type Activity as ActivityItem, type Account, type RepoDetails, type LoginCode, type WorkspaceSettings, type Discovery } from './lib/api';

  let repos = $state<Repo[]>([]);

  let activities = $state<ActivityItem[]>([]);

  let accounts = $state<Account[]>([]);

  let view = $state('all');

  let query = $state('');

  let selected = $state<number | null>(null);

  let details = $state<RepoDetails | null>(null);

  let working = $state(false);

  let loading = $state(true);

  let notice = $state('');

  let error = $state('');

  let showAdd = $state(false);
  let showWalkthrough = $state(false);
  let workspace=$state<WorkspaceSettings>({default_folder:null,auto_detect:false,setup_complete:false});
  let showSetup=$state(false),setupBusy=$state(false),showImport=$state(false);
  let found=$state<Discovery|null>(null),importInitial=$state<Discovery|null>(null);
  let discoverySequence=0;let detectionBusy=false;let discoveryError=$state('');
  async function scanDefault(){if(detectionBusy||!workspace.auto_detect||!workspace.default_folder)return;detectionBusy=true;const sequence=++discoverySequence;try{const result=await api.discover('default_folder');if(sequence===discoverySequence){found=result;discoveryError='';}}catch(e){if(sequence===discoverySequence)discoveryError=String(e);}finally{detectionBusy=false;}}
  async function workspaceSaved(settings:WorkspaceSettings){workspace=settings;showSetup=false;++discoverySequence;found=null;discoveryError='';void scanDefault();if(!await api.walkthroughSeen())showWalkthrough=true;}
  async function skipSetup(){if(setupBusy)return;setupBusy=true;try{await workspaceSaved(await api.saveWorkspaceSettings({...workspace,setup_complete:true}));}catch(e){error=String(e);}finally{setupBusy=false;}}
  function openImport(initial:Discovery|null=null){importInitial=initial;showImport=true;}
  async function imported(count:number){showImport=false;found=null;notice=`Imported ${count} repositories`;await load();}


  let showAccount = $state(false);

  let showOrganize = $state(false);


  let token = $state('');
  let accountMode = $state<'browser'|'token'>('browser');
  let browserAvailable = $state<boolean | null>(null);
  let privateRepos = $state(false);
  let loginCode = $state<LoginCode | null>(null);
  let loginBusy = $state(false);
  let loginError = $state('');
  let loginMessage = $state('');
  let loginSequence = 0;
  let loginWake: (() => void) | null = null;

  let collection = $state<string[]>([]);

  let tags = $state<string[]>([]);
  let showBranch = $state(false);
  let conflictFile = $state<string|null>(null);
  let pendingGit = $state<{id:number;action:string;args:string[];title:string;description:string;expected:{branch:string;head:string|null}}|null>(null);

  let message = $state('');

  let newBranch = $state('');

  let activeTab = $state<'changes'|'history'|'pulls'>('changes');

  let actionResult = $state('');

  let refreshing = $state(false);

  let detailError = $state('');
  let fileQuery = $state('');
  let selectedFile = $state<string | null>(null);
  let diffCode = $state('');
  let diffLoading = $state(false);
  let diffError = $state('');
  let diffSequence = 0;
  const commitDrafts = new Map<number, string>();

  let loadFlight: Promise<void> | null = null;

  let selectionSequence = 0;

  let searchInput = $state<HTMLInputElement>();

  let page = $state(1);

  const pageSize = 50;

  $effect(() => { query; view; page = 1; });
  $effect(() => { if(selected!==null)commitDrafts.set(selected,message); });

  const current = $derived(repos.find(r => r.id === selected) ?? null);
  const gitState = $derived(details?.git??null);
  const pushBlocked = $derived(working||!gitState?.has_head||gitState.dirty>0||gitState.operation!==null||gitState.conflicts.length>0||!gitState.remote||gitState.upstream_gone);

  const visible = $derived(filteredRepos(repos, query, view));

  const fileNeedle = $derived(fileQuery.toLocaleLowerCase());
  const fileMatches = $derived((details?.changes ?? []).filter(file => file.path.toLocaleLowerCase().includes(fileNeedle)));
  const repoActivity = $derived(current ? activities.filter(item => item.repo_id === current.id).slice(0, 3) : []);
  const attention = $derived(repos.filter(r => r.status_error || r.dirty || r.behind || r.ci_conclusion === 'failure').length);

  const pinned = $derived(repos.filter(r => r.pinned).length);

  const collections = $derived([...new Set(repos.map(r => r.collection).filter((x): x is string => !!x))]);

  const date = (sec:number) => new Date(sec*1000).toLocaleString(undefined,{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit'});

  const short = (value: string, max = 45) => value.length > max ? '…' + value.slice(-(max-1)) : value;

  async function load() {

    if (loadFlight) return loadFlight;

    loadFlight = (async () => {

      const results = await Promise.allSettled([api.repos(), api.activity(), api.accounts()]);

      if (results[0].status === 'fulfilled') repos = results[0].value;

      if (results[1].status === 'fulfilled') activities = results[1].value;

      if (results[2].status === 'fulfilled') accounts = results[2].value;

      const failed = results.find(r => r.status === 'rejected');

      if (failed?.status === 'rejected') error = String(failed.reason);

      loading = false;

    })().finally(() => { loadFlight = null; });

    return loadFlight;

  }

  function beginSelection(id: number) {
    const sequence = ++selectionSequence;
    if (selected !== null) commitDrafts.set(selected, message);
    selected = id; message = commitDrafts.get(id) ?? ''; newBranch = ''; details = null; detailError = ''; activeTab = 'changes'; actionResult = '';
    fileQuery = ''; selectedFile = null; diffCode = ''; diffError = ''; diffLoading = false; ++diffSequence;
    return sequence;
  }
  async function selectRepo(id:number) {
    const sequence = beginSelection(id);
    try { const result = await api.details(id); if (sequence === selectionSequence && selected === id) { details = result; if (result.changes[0]) void showDiff(result.changes[0].path, id); } }

    catch(e) { if (sequence === selectionSequence) detailError = String(e); }

  }

  async function act(fn:()=>Promise<unknown>,msg:string,refreshWorkspace=true) {

    if (working) return false;

    const actionRepo = selected;
    working=true; error='';

    try {

      const result=await fn(); notice=msg; if (selected === actionRepo) actionResult=typeof result==='string'?result:'';

      if(refreshWorkspace)await load();else void api.activity().then(rows=>activities=rows).catch(()=>{});

      const id=selected;

      if(id!==null) { try { const result=await api.details(id); if(selected===id){details=result;updateRepoStatus(id,result);const file=result.changes.some(file=>file.path===selectedFile)?selectedFile:result.changes[0]?.path??null;void showDiff(file,id);} } catch(e) {detailError=String(e);} }

      return true;

    } catch(e) {error=String(e);if(refreshWorkspace)await load();const id=selected;if(id!==null){try{const result=await api.details(id);if(selected===id){details=result;updateRepoStatus(id,result);}}catch{}}return false;} finally {working=false;}

  }

  async function repositoryAdded(repo:Repo,warning?:string|null){showAdd=false;notice='Repository ready';error=warning??'';await load();if(!repos.some(r=>r.id===repo.id))repos=[...repos,repo];await selectRepo(repo.id);}
  function openAdd(){showAdd=true;}
  function closeAdd(){showAdd=false;}
  async function prepareTour(target:string){
    if(target==='commit'&&repos[0]){await selectRepo(repos[0].id);activeTab='changes';}
    else{selected=null;view=target==='connect'?'settings':'all';}
    await tick();
  }
  function updateRepoStatus(id:number,result:RepoDetails){repos=repos.map(repo=>repo.id===id?{...repo,branch:result.git.branch,dirty:result.git.dirty,ahead:result.git.ahead,behind:result.git.behind,status_error:null}:repo);}
  async function refresh(){
    if(refreshing||working)return;refreshing=true;error='';const sequence=selectionSequence;
    try{const id=selected;if(id===null)await load();if(id!==null){const result=await api.details(id);if(selected===id&&sequence===selectionSequence){details=result;updateRepoStatus(id,result);detailError='';if(activeTab==='changes'){const file=result.changes.some(item=>item.path===selectedFile)?selectedFile:result.changes[0]?.path??null;void showDiff(file,id);}}}}
    catch(e){detailError=String(e);}finally{refreshing=false;}
  }

  async function openOrganization(repo: Repo | undefined = current ?? repos[0]) {
    if(working)return;
    if(!repo){openAdd();return;}
    if(selected!==repo.id)await selectRepo(repo.id);
    if(working||selected!==repo.id)return;
    collection=repo.collection?[repo.collection]:[];tags=[...repo.tags];showOrganize=true;
  }
  async function organize(){if(!current)return;if(await act(()=>api.organize(current.id,collection[0]??'',tags),'Collection updated'))showOrganize=false;}

  function openAccount(){accountMode='browser';privateRepos=false;loginError='';loginMessage='';showAccount=true;}
  function closeAccount(){
    ++loginSequence;showAccount=false;token='';loginError='';loginMessage='';
    loginWake?.();
    const id=loginCode?.id;loginCode=null;
    if(id!==undefined)void api.cancelLogin(id).catch(()=>{error='Could not cancel GitHub sign-in. Restart Reponiq before trying again.';});
  }
  function waitForLogin(ms: number){return new Promise<void>(resolve=>{const timer=setTimeout(()=>{loginWake=null;resolve();},ms);loginWake=()=>{clearTimeout(timer);loginWake=null;resolve();};});}
  async function reopenLogin(){if(!loginCode)return;try{await api.openLogin(loginCode.id);loginError='';}catch(e){loginError=String(e);}}
  async function browserConnect(){
    if(loginBusy||working||!browserAvailable)return;
    const sequence=++loginSequence;loginBusy=true;loginError='';token='';
    let id: number | undefined;
    try{
      const code=await api.startLogin(privateRepos);id=code.id;
      if(sequence!==loginSequence||!showAccount){await api.cancelLogin(code.id);return;}
      loginCode=code;
      await reopenLogin();
      await waitForLogin(code.interval*1000);
      while(sequence===loginSequence&&showAccount){
        const result=await api.pollLogin(code.id);
        if(sequence!==loginSequence||!showAccount)break;
        if(result.status==='connected'){
          notice='Connected @'+result.account.login;loginCode=null;closeAccount();await load();return;
        }
        loginMessage=result.message??'Waiting for GitHub…';
        await waitForLogin(Math.max(1,result.retry_after)*1000);
      }
    }catch(e){if(sequence===loginSequence&&showAccount){loginError=String(e);loginCode=null;}}
    finally{if(id!==undefined)void api.cancelLogin(id).catch(()=>{});loginBusy=false;void load();}
  }
  async function connect(){if(!token.trim()||loginBusy)return;if(await act(()=>api.connect(token.trim()),'GitHub account connected'))closeAccount();}

  async function sync(){await act(async()=>{const results=[];for(const a of accounts)results.push(await api.sync(a.id));return results.join('\n');},'GitHub workflows synchronized');}

  async function runGit(action:string,args:string[]=[]){if(!current)return false;const id=current.id;const expected=gitState?{branch:gitState.branch,head:gitState.head}:null;return act(()=>api.git(id,action,args,expected),`Git ${action.replaceAll('_',' ')} completed`,false);}

  async function commit(){const id=selected;if(await runGit('commit',[message])){if(id!==null)commitDrafts.delete(id);if(selected===id)message='';}}

  async function createBranch(){const id=selected;const name=newBranch;if(await runGit('create_branch',[name])&&selected===id&&newBranch===name)newBranch='';}
  async function branchChanged(event: Event){const field=event.currentTarget as HTMLSelectElement;const branch=field.value;field.value=current?.branch??'';if(!branch){newBranch='';showBranch=true;return;}await runGit('checkout',[branch]);}
  function revertCommit(hash:string,merge:boolean){queueGit('revert','Revert commit?', 'Create a new commit that undoes '+hash.slice(0,8)+(merge?' relative to its first parent':'')+'. Existing history will be preserved.',[hash]);}
  function queueGit(action:string,title:string,description:string,args:string[]=[]){if(!current||!gitState||working)return;pendingGit={id:current.id,action,args,title,description,expected:{branch:gitState.branch,head:gitState.head}};}
  async function executePendingGit(){if(!pendingGit)return;const task=pendingGit;pendingGit=null;await act(()=>api.git(task.id,task.action,task.args,task.expected),task.action==='revert'?'Revert commit created':'Git action completed',false);}

  async function removeRepo(repo: Repo) {
    const id = repo.id;
    if(!window.confirm('Remove '+repo.name+' from Reponiq? This does not delete files.'))return;
    await act(async()=>{await api.remove(id);commitDrafts.delete(id);if(selected===id){selected=null;details=null;message='';diffLoading=false;++diffSequence;}},'Repository removed');
  }
  async function showDiff(file: string | null, id = selected) {
    if (id === null) return;
    const sequence = ++diffSequence;
    selectedFile = file; diffCode = ''; diffError = ''; diffLoading = true;
    try { const code = await api.git(id, 'diff', file === null ? [] : [file]); if (sequence === diffSequence && selected === id) diffCode = code; }
    catch(e) { if(sequence === diffSequence && selected === id) diffError=String(e); }
    finally { if(sequence === diffSequence)diffLoading=false; }
  }
  async function keyboard(event: KeyboardEvent){

    const editing=event.target instanceof HTMLInputElement||event.target instanceof HTMLTextAreaElement;
    if(document.querySelector('dialog[open]'))return;

    if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='k'&&!showAdd&&!showAccount&&!showOrganize&&!showBranch&&!conflictFile){event.preventDefault();selected=null;view='all';await tick(); searchInput?.focus();}

    if(event.key==='Escape'&&!editing&&!showAdd&&!showAccount&&!showOrganize&&!showBranch&&!conflictFile){selected=null;details=null;}

  }

  function confirmGit(action:string){const labels:{[k:string]:[string,string]}={pull:['Pull changes?','Fetch and fast-forward this branch from its upstream.'],push:['Push branch?','Push the committed branch to its tracking remote.'],merge_upstream:['Merge upstream?','Combine the latest upstream with this branch without rewriting history. Conflicts may need to be resolved.'],abort_operation:['Abort Git operation?','Resolution work from this operation may be discarded. Git will attempt to restore the pre-operation state.'],unstage_all:['Unstage all files?','Keep the working files and remove them from the next commit.']};const label=labels[action];if(label)queueGit(action,...label);else void runGit(action);}

  onMount(()=>{void load().then(()=>api.workspaceSettings()).then(async settings=>{workspace=settings;if(!settings.setup_complete)showSetup=true;else{if(!await api.walkthroughSeen())showWalkthrough=true;void scanDefault();}}).catch(e=>error=String(e));void api.browserLoginAvailable().then(value=>browserAvailable=value).catch(()=>browserAvailable=false);const timer=setInterval(()=>{if(document.visibilityState==='visible'&&!working&&!document.querySelector('dialog[open]')&&!showAdd&&!showAccount&&!showOrganize){void load();void scanDefault();}},60000);return()=>{clearInterval(timer);closeAccount();}});

</script>



<svelte:window onkeydown={keyboard}/>

<svelte:head><title>Reponiq — Repository workspace</title><meta name="description" content="A local-first Git and GitHub dashboard"/></svelte:head>

<a class="skip-link" href="#main-content">Skip to workspace</a>

<div class="app-shell">

  <aside class="sidebar">

    <div class="brand"><div class="brand-mark"><svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M6 19V5h7a4 4 0 0 1 0 8H6m7 0 5 6" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/><circle cx="18" cy="19" r="2" fill="currentColor" stroke="none"/></svg></div><div><div class="brand-title">Reponiq</div></div></div>

    <div class="side-section">WORKSPACE</div>

    <button title="Overview" class:active={view==='all' && selected===null} onclick={()=>{selected=null;view='all'}}><LayoutDashboard size={17}/> Overview</button>

    <button title="Pinned repositories" class:active={view==='pinned' && selected===null} onclick={()=>{selected=null;view='pinned'}}><Pin size={17}/> Pinned <span class="side-count">{pinned}</span></button>

    <button title="Needs attention" class:active={view==='attention' && selected===null} onclick={()=>{selected=null;view='attention'}}><CircleAlert size={17}/> Needs attention <span class="side-count">{attention}</span></button>

    <button title="Activity history" class:active={view==='activity'} onclick={()=>{selected=null;view='activity'}}><History size={17}/> Activity history</button>

    <div class="side-head"><span class="side-section">COLLECTIONS</span><button class="icon-only" title="Add collection when organizing a repository" disabled={working} onclick={()=>openOrganization()}><Plus size={15}/></button></div>

    {#each collections as c}<button class:active={view==='collection:'+c} onclick={()=>{selected=null;view='collection:'+c}}><Folder size={17}/>{c}</button>{/each}

    {#if collections.length===0}<div class="muted-sidebar">Organize repositories into groups</div>{/if}

    <div class="sidebar-grow"></div>

    <div class="sidebar-bottom">

      <button title="Settings" onclick={()=>{selected=null;view='settings'}} class:active={view==='settings'}><Settings size={17}/> Settings</button>

      <div class="connection"><span class="live-dot"></span> Local workspace <span class="version">v1.0</span></div>

    </div>

  </aside>

  <div class="main-area">


    <main class="content" class:workbench-content={!!current} id="main-content" tabindex="-1" aria-busy={loading||working}>

      {#if error}<div class="alert error" role="alert"><CircleAlert size={16}/><span>{error}</span><button title="Dismiss error" onclick={()=>error=''}><X size={15}/></button></div>{/if}

      {#if notice}<div class="alert success" role="status"><Check size={16}/><span>{notice}</span><button title="Dismiss notification" onclick={()=>notice=''}><X size={15}/></button></div>{/if}

      {#if current}
        <div class="workbench-shell">

          <section class="workbench-main" aria-label="Git workbench">
            <div class="page-heading workbench-heading">
              <div><button class="back-link" onclick={()=>{selected=null;details=null}}><ArrowLeftRight size={14}/> All repositories</button><div class="heading-row"><PlatformIcon remote={current.remote_url} size={25}/><h1>{current.name}</h1></div><p title={current.path}>{current.path}</p></div>
              <div class="heading-actions"><button class="secondary" aria-label={current.pinned?'Unpin repository':'Pin repository'} disabled={working} onclick={()=>act(()=>api.pin(current.id,!current.pinned),current.pinned?'Unpinned':'Pinned')}><Pin size={16}/></button><button class="secondary" disabled={working} onclick={()=>openOrganization(current)}><Tags size={15}/> Organize</button></div>
            </div>
            <div class="workbench-tools">
              <label class="branch-control"><GitBranch size={16}/><select aria-label="Current branch" value={current.branch} onchange={branchChanged} disabled={working||!details||!!gitState?.operation}>{#if !details?.branches.includes(current.branch)}<option value={current.branch}>{current.branch}</option>{/if}{#each details?.branches??[] as branch}<option value={branch}>{branch}</option>{/each}<option value="">+ Create branch…</option></select></label>
              <div class="toolbar"><button title="Refresh status" aria-label="Refresh status" onclick={refresh} disabled={refreshing||working}><RefreshCw size={14}/></button><button onclick={()=>runGit('fetch')} disabled={working||!!current.status_error||!gitState?.remotes.length}><RefreshCw size={14}/> Fetch</button><button onclick={()=>confirmGit('pull')} title={gitState?.upstream??'Publish this branch to set an upstream'} disabled={working||!!current.status_error||!gitState?.upstream||!!gitState?.dirty||!!gitState?.operation}><ArrowDown size={14}/> Pull</button><button class="primary" onclick={()=>confirmGit('push')} title={gitState?.dirty?'Commit all changes before pushing':!gitState?.remote?'No tracking remote configured':'Push committed changes'} disabled={pushBlocked||!!current.status_error}><ArrowUp size={14}/> {gitState?.upstream?'Push':'Publish branch'}</button></div>
            </div>
            <div class="workbench-status"><span class="status-label" class:status-warning={current.dirty>0||current.behind>0||!!current.status_error}><i></i>{current.status_error?'Unavailable':current.dirty?'Changed':'Clean'}</span><span><ArrowUp size={13}/>{current.ahead} ahead</span><span><ArrowDown size={13}/>{current.behind} behind</span>{#if current.ci_conclusion||current.ci_status}<span class="workbench-ci"><PlatformIcon remote="https://github.com" size={14}/>{current.ci_conclusion??current.ci_status}</span>{/if}</div>
            {#if gitState?.operation||gitState?.conflicts.length}<section class="operation-banner" aria-label="Git operation"><div><strong>{gitState.operation??'Git operation'} paused</strong><span>{gitState.conflicts.length?gitState.conflicts.length+' conflicting files':'Conflicts resolved. Continue to finish.'}</span></div><div class="toolbar"><button class="primary" disabled={working||gitState.conflicts.length>0} onclick={()=>runGit('continue_operation')}>Continue</button><button class="secondary dangerous" disabled={working} onclick={()=>confirmGit('abort_operation')}>Abort</button></div></section>{:else if gitState&&gitState.ahead>0&&gitState.behind>0}<section class="operation-banner"><div><strong>Branches have diverged</strong><span>{gitState.ahead} local · {gitState.behind} upstream commits</span></div><button class="secondary" disabled={working||gitState.dirty>0} onclick={()=>confirmGit('merge_upstream')}>Merge upstream</button></section>{/if}
            {#if gitState?.dirty&&!gitState.operation}<p class="push-hint">Commit or ignore all changes before pushing.</p>{/if}
            <div class="tabs" aria-label="Repository views"><button class:tab-active={activeTab==='changes'} onclick={()=>activeTab='changes'}>Changes <span>{current.dirty}</span></button><button class:tab-active={activeTab==='history'} onclick={()=>activeTab='history'}>Commit history</button>{#if current.github_full_name}<button class:tab-active={activeTab==='pulls'} onclick={()=>activeTab='pulls'}>Pull requests</button>{/if}</div>
            {#if detailError}
              <div class="empty-block" role="alert"><CircleAlert size={20}/><span>{detailError}</span><button class="secondary" onclick={()=>current&&selectRepo(current.id)}>Retry</button></div>
            {:else if !details}<div class="empty-block"><LoaderCircle class="spin"/> Loading Git details...</div>
            {:else if activeTab==='changes'}
              <div class="changes-workbench">
                <section class="changed-files panel" aria-label="Changed files">
                  <div class="panel-title">Changed files <span>{details.changes.length}</span></div>
                  {#if details.changes.length>8}<div class="search-input file-search"><Search size={13}/><input aria-label="Filter changed files" placeholder="Filter files" bind:value={fileQuery}/></div>{/if}
                  <div class="file-list">
                    {#each fileMatches.slice(0,200) as file (file.path)}
                      <div class="file-change-row"><input type="checkbox" aria-label={'Stage '+file.path} checked={file.x!==' '&&file.x!=='?'} disabled={working||!!gitState?.conflicts.includes(file.path)} onchange={()=>runGit(file.x!==' '&&file.x!=='?'?'unstage_file':'stage_file',[file.path])}/><button class="changed-file" class:file-selected={selectedFile===file.path} aria-pressed={selectedFile===file.path} title={file.path} onclick={()=>{if(selectedFile!==file.path)void showDiff(file.path,current.id);}}><FileCode size={15}/><span>{file.path}</span><b class:change-added={file.x==='A'||file.x==='?'} class:change-deleted={file.x==='D'||file.y==='D'} class:change-conflict={gitState?.conflicts.includes(file.path)}>{(file.x+file.y).trim()}</b></button>{#if gitState?.conflicts.includes(file.path)}<button class="resolve-file" onclick={()=>conflictFile=file.path}>Resolve</button>{/if}</div>
                    {:else}<div class="empty-small"><Check size={17}/>{details.changes.length?'No matching files':'Working tree clean'}</div>{/each}
                  </div>
                  {#if fileMatches.length>200}<small class="rail-limit">200 of {fileMatches.length} files. Filter to find a file.</small>{/if}
                </section>
                <section class="diff-panel panel" aria-label="File diff" aria-busy={diffLoading}>
                  <div class="panel-title"><span class="diff-filename" title={selectedFile??''}><FileCode size={15}/>{selectedFile??'Working tree'}</span><button class="text-action" disabled={diffLoading} onclick={()=>showDiff(null,current.id)}>All changes</button></div>
                  {#if diffLoading}<div class="diff-empty"><LoaderCircle class="spin" size={20}/><span>Loading diff...</span></div>
                  {:else if diffError}<div class="diff-empty" role="alert"><CircleAlert size={20}/><span>{diffError}</span><button class="secondary" onclick={()=>showDiff(selectedFile,current.id)}>Retry</button></div>
                  {:else if diffCode}
                  <DiffView code={diffCode} label="Git diff"/>
                  {:else}<div class="diff-empty"><Code2 size={25}/><span>{details.changes.some(file=>file.path===selectedFile&&file.x==='?')?'Untracked file. Stage it to review the diff.':details.changes.length?'No textual diff. The file may be binary or unchanged.':'No changes'}</span></div>{/if}
                </section>
              </div>
              <div data-tour="commit" class="commit-panel workbench-commit"><div class="commit-heading">Commit changes</div><textarea disabled={working} aria-label="Commit message" placeholder="Commit message" bind:value={message} rows="2" maxlength="500"></textarea><div class="toolbar inner"><button onclick={()=>runGit('stage_all')} disabled={working||!!current.status_error}>Stage all</button><button onclick={()=>confirmGit('unstage_all')} disabled={working||!!current.status_error}>Unstage all</button><button class="primary" onclick={commit} disabled={working||!message.trim()||!!current.status_error||!gitState?.staged||!!gitState.operation}>Commit {gitState?.staged??0} staged files</button></div></div>
            {:else if activeTab==='history'}
              {#key current.id}<CommitHistory repoId={current.id} head={gitState?.head??null} blocked={working||!!gitState?.dirty||!!gitState?.operation} onrevert={revertCommit}/>{/key}
            {:else if activeTab==='pulls'}
              {#key current.id}<PullRequests repoId={current.id} git={gitState} {accounts} onconnect={openAccount} onchanged={()=>void refresh()}/>{/key}
            {/if}
            {#if actionResult}<div class="panel"><div class="panel-title">Git output</div><pre class="output">{actionResult}</pre></div>{/if}
            {#if repoActivity.length}<div class="workbench-activity panel"><div class="panel-title">Recent activity</div>{#each repoActivity as item}<div class="activity-entry"><div class="activity-icon"><Activity size={15}/></div><div class="activity-text"><strong>{item.title}</strong><span>{short(item.detail??'',70)}</span></div><time>{date(item.occurred_at)}</time></div>{/each}</div>{/if}
            <button class="remove-repo" disabled={working} onclick={()=>removeRepo(current)}><X size={13}/> Remove from workspace</button>
          </section>
        </div>
      {:else if view==='settings'}

        <div class="page-heading"><div><h1>Settings</h1><p>Manage GitHub connections and local configuration.</p></div></div>

        <div class="panel"><div class="panel-title">GitHub accounts <span>{accounts.length} connected</span></div>{#each accounts as acc}<div class="account-row"><div class="account-avatar"><PlatformIcon remote="https://github.com" size={19}/></div><div class="account-info"><strong>@{acc.login}</strong><small>{acc.api_host}</small></div><button class="micro" onclick={()=>act(()=>api.sync(acc.id),'Synchronized '+acc.login)}>Sync workflows</button><button class="micro dangerous" onclick={()=>{if(window.confirm('Disconnect @'+acc.login+' and delete its saved token?'))void act(()=>api.disconnect(acc.id),'Account disconnected')}}>Disconnect</button></div>{:else}<div class="empty-small">No GitHub accounts connected. Local Git works without one.</div>{/each}<div class="panel-footer"><button data-tour="connect" class="primary" onclick={openAccount}><Plus size={15}/> Connect account</button></div></div>

        <div class="panel info-panel"><ShieldCheck size={21}/><div><strong>Credentials stay on your device</strong><p>GitHub access tokens are stored in your operating system's credential manager. Repository metadata, pins, and activity are stored locally in SQLite. Account connections support GitHub.com; local Git works with other hosts.</p></div></div>

        <div class="panel workspace-settings"><h2>Repository folder</h2><WorkspaceFolder settings={workspace} onsaved={settings=>{workspace=settings;++discoverySequence;found=null;discoveryError='';void scanDefault();}}/></div>
        <div class="panel walkthrough-settings"><div><strong>A quick tour</strong><p>Revisit the basics whenever you need them.</p></div><button class="secondary" onclick={()=>showWalkthrough=true}>Open walkthrough</button></div>

      {:else if view==='activity'}

        <div class="page-heading"><div><h1>Activity history</h1><p>Local actions and observed GitHub workflow runs.</p></div><button class="secondary" onclick={sync} disabled={working||accounts.length===0}><RefreshCw size={15}/> Sync GitHub</button></div>

        <div class="panel"><div class="panel-title">Recent events <span>{activities.length} events</span></div>{#each activities as item}<div class="activity-entry"><div class="activity-icon"><Activity size={15}/></div><div class="activity-text"><strong>{item.title}</strong><span>{item.repo_name??'GitHub'}{item.detail?' · '+short(item.detail,110):''}</span></div><time>{date(item.occurred_at)}</time></div>{:else}<div class="empty-small">No activity yet.</div>{/each}</div>

      {:else}

        <div class="page-heading"><div><h1 class="overview-title">{view==='pinned'?'Pinned repositories':view==='attention'?'Needs attention':view.startsWith('collection:')?view.slice(11):'Overview'}</h1></div><div class="heading-actions"><button class="secondary" onclick={()=>openImport()}>Import from…</button><button data-tour="add" class="primary" disabled={working} onclick={openAdd}><Plus size={16}/> Add repository</button></div></div>

        {#if discoveryError}<div class="discovery-banner" role="alert"><span>{discoveryError}</span><button class="secondary" onclick={()=>view='settings'}>Choose folder</button></div>{:else if found?.items.length}<div class="discovery-banner" role="status"><span>{found.items.length} new repositories found</span><button class="secondary" onclick={()=>openImport(found)}>Review & import</button></div>{/if}
        <div class="shelf-tools">
          <div class="collection-chips" aria-label="Filter by collection"><button class:chip-active={view==='all'} onclick={()=>view='all'}>All</button>{#each collections as c}<button class:chip-active={view==='collection:'+c} onclick={()=>view='collection:'+c}><Folder size={13}/>{c}</button>{/each}</div>
          <div class="search-input shelf-search"><Search size={17}/><input data-tour="search" bind:this={searchInput} aria-label="Search repositories" placeholder="Search repositories" bind:value={query}/><kbd class="key-hint">Ctrl K</kbd></div>
          <button class="secondary shelf-refresh" title="Refresh repositories" aria-label="Refresh repositories" onclick={refresh} disabled={refreshing||working}><RefreshCw size={16}/></button>
        </div>
        <div class="shelf-count">{visible.length} {visible.length===1?'repository':'repositories'}</div>
        <div class="repository-shelf" aria-label="Repositories">
          {#each visible.slice((page-1)*pageSize,page*pageSize) as r}
            <article class="repository-card">
              <div class="card-heading"><button class="card-open" onclick={()=>selectRepo(r.id)}><span class="card-icon"><PlatformIcon remote={r.remote_url} size={25}/></span><h2>{r.name}</h2></button><button class="card-pin" class:is-pinned={r.pinned} aria-label={r.pinned?'Unpin '+r.name:'Pin '+r.name} title={r.pinned?'Unpin':'Pin'} disabled={working} onclick={()=>act(()=>api.pin(r.id,!r.pinned),r.pinned?'Unpinned':'Pinned')}><Pin size={17}/></button></div>
              <p class="card-path" title={r.path}><span class="platform-name">{remoteInfo(r.remote_url).label}</span> · {remoteInfo(r.remote_url).project||short(r.path,64)}</p>
              <div class="card-state"><span class="card-branch" title={r.branch}><GitBranch size={14}/>{r.branch}</span>{#if r.status_error}<span class="pill red" title={r.status_error}>Unavailable</span>{:else if r.dirty>0}<span class="pill yellow">{r.dirty} changed</span>{:else if r.ci_conclusion==='failure'}<span class="pill red">CI failed</span>{:else}<span class="pill green"><Check size={12}/> Clean</span>{/if}</div>
              <div class="card-counts"><span><ArrowUp size={13}/>{r.ahead} ahead</span><span><ArrowDown size={13}/>{r.behind} behind</span>{#if r.collection}<span class="card-collection" title={r.collection}>{r.collection}</span>{/if}</div>
              <footer class="card-footer">{#if activities.find(item=>item.repo_id===r.id)}<span><Clock3 size={12}/>{date(activities.find(item=>item.repo_id===r.id)!.occurred_at)}</span>{:else}<span class="card-tags">{r.tags.slice(0,3).join(' / ')}</span>{/if}{#if r.ci_conclusion||r.ci_status}<span class="card-ci"><Activity size={12}/>{r.ci_conclusion??r.ci_status}</span>{/if}</footer>
            </article>
          {:else}<div class="empty-large shelf-empty"><div class="empty-illustration"><FolderGit2 size={32} strokeWidth={1.5}/></div><h3>{loading?'Opening workspace...':repos.length===0?'No repositories yet':'No repositories found'}</h3><p>{repos.length===0?'Add an existing local Git repository.':'Try a different search or collection.'}</p><button data-tour="add" class="primary" disabled={working} onclick={openAdd}><Plus size={16}/> Add repository</button></div>{/each}
        </div>
        {#if visible.length>pageSize}<div class="pagination"><button class="secondary" disabled={page===1} onclick={()=>page--}>Previous</button><span>Page {page} of {Math.ceil(visible.length/pageSize)}</span><button class="secondary" disabled={page*pageSize>=visible.length} onclick={()=>page++}>Next</button></div>{/if}

        <section class="latest-activity panel" aria-label="Latest activity"><div class="latest-heading"><h2><Clock3 size={17}/> Latest activity</h2><button class="text-action" onclick={()=>view='activity'}>View all activity <ArrowUp size={13}/></button></div><div class="latest-items">{#each activities.slice(0,3) as item}<div class="latest-item"><span class="activity-icon">{#if item.kind==='ci'}<Activity size={16}/>{:else}<GitCommitHorizontal size={16}/>{/if}</span><div><strong>{item.repo_name??'GitHub'} &middot; {item.title}</strong><small>{date(item.occurred_at)}</small></div></div>{:else}<div class="empty-small">No activity yet</div>{/each}</div></section>
      {/if}

    </main>

  </div>

</div>

{#if showAdd}<AddRepository {accounts} defaultFolder={workspace.default_folder} onclose={closeAdd} onconnect={()=>{closeAdd();openAccount();}} oncreated={repositoryAdded}/>{/if}

{#if showOrganize}<Modal label="Organize repository" onclose={()=>{showOrganize=false;}}><div class="modal-top"><div><h2>Organize {current?.name??'repository'}</h2></div><button class="top-icon" aria-label="Close dialog" onclick={()=>showOrganize=false}><X size={18}/></button></div><TokenInput label="Collection" bind:items={collection} maxItems={1} maxLength={100}/><TokenInput label="Tags" bind:items={tags}/><div class="modal-actions"><button class="secondary" onclick={()=>showOrganize=false}>Cancel</button><button class="primary" onclick={organize} disabled={working}>Save changes</button></div></Modal>{/if}

{#if showAccount}<Modal label="Connect GitHub" onclose={closeAccount}><div class="modal-top"><div><h2>Connect GitHub</h2></div><button class="top-icon" aria-label="Close dialog" onclick={closeAccount}><X size={18}/></button></div>
  {#if loginError}<p class="connection-error" role="alert">{loginError}</p>{/if}
  {#if accountMode==='browser'}
    {#if loginCode}<p>Enter this code on GitHub:</p><div class="device-code" aria-label="GitHub sign-in code">{loginCode.user_code}</div><p class="login-status" role="status"><LoaderCircle size={15} class="spin"/>{loginMessage||'Waiting for GitHub…'}</p><button class="secondary browser-reopen" onclick={reopenLogin}>Open GitHub</button>
    {:else}<p>Continue in your browser to connect your account and manage public repositories and pull requests.</p>{#if browserAvailable===false}<p class="connection-error">Browser sign-in needs app setup. You can use a token below.</p>{/if}<small>GitHub access includes public repository writes and organization membership.</small><label class="scope-choice"><input type="checkbox" bind:checked={privateRepos} disabled={loginBusy}/><span>Include private repositories</span></label>{#if privateRepos}<small>GitHub's repository permission also grants write access. Reponiq uses this connection for repositories, workflows and pull requests.</small>{/if}<div class="modal-actions"><button class="secondary" onclick={closeAccount}>Cancel</button><button class="primary" onclick={browserConnect} disabled={loginBusy||working||!browserAvailable}>{#if loginBusy}<LoaderCircle size={15} class="spin"/>{:else}<Github size={16}/>{/if} Continue with GitHub</button></div><button class="text-action auth-alternative" disabled={loginBusy} onclick={()=>{accountMode='token';loginError='';}}>Use a personal access token</button>{/if}
  {:else}<label for="token">Personal access token</label><input id="token" type="password" placeholder="github_pat_…" autocomplete="off" bind:value={token}/><small>For private workflows, use a fine-grained token with Actions read access.</small><div class="modal-actions"><button class="secondary" onclick={()=>{accountMode='browser';token='';loginError='';}}>Back</button><button class="primary" onclick={connect} disabled={working||!token.trim()}>Connect account</button></div>{/if}
</Modal>{/if}

{#if showBranch}<Modal label="Create branch" onclose={()=>showBranch=false}><h2>Create branch</h2><label for="branch-name">Branch name</label><input id="branch-name" bind:value={newBranch} placeholder="feature/name" onkeydown={event=>{if(event.key==='Enter'&&!event.isComposing)void createBranch().then(()=>{if(!newBranch)showBranch=false;});}}/><div class="modal-actions"><button class="secondary" onclick={()=>showBranch=false}>Cancel</button><button class="primary" disabled={working||!newBranch.trim()} onclick={()=>createBranch().then(()=>{if(!newBranch)showBranch=false;})}>Create & switch</button></div></Modal>{/if}
{#if conflictFile&&current}{#key current.id+':'+conflictFile}<ConflictEditor repoId={current.id} file={conflictFile} expected={gitState?{branch:gitState.branch,head:gitState.head}:null} onclose={()=>conflictFile=null} onsaved={()=>void refresh()}/>{/key}{/if}

{#if pendingGit}<Modal label={pendingGit.title} onclose={()=>pendingGit=null}><h2>{pendingGit.title}</h2><p class="confirm-description">{pendingGit.description}</p><p class="confirm-context">{repos.find(repo=>repo.id===pendingGit!.id)?.name} · {pendingGit.expected.branch}</p><div class="modal-actions"><button class="secondary" onclick={()=>pendingGit=null}>Cancel</button><button class="primary" onclick={executePendingGit}>Confirm</button></div></Modal>{/if}

{#if showWalkthrough}<Walkthrough hasRepo={repos.length>0} prepare={prepareTour} onclose={()=>showWalkthrough=false}/>{/if}

{#if showSetup}<Modal label="Set up repositories" canClose={!setupBusy} onclose={skipSetup}><h2>Choose your repository folder</h2><p>You can change this later in Settings.</p><WorkspaceFolder settings={workspace} initial bind:busy={setupBusy} onsaved={workspaceSaved}/></Modal>{/if}
{#if showImport}<ImportRepositories initial={importInitial} defaultFolder={workspace.default_folder} onclose={()=>{showImport=false;void load();}} onimported={imported}/>{/if}
