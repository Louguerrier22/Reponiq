<script lang="ts">
  import Button from './lib/ui/Button.svelte';
  import ConfirmDialog from './lib/ui/ConfirmDialog.svelte';
  import { confirm } from './lib/ui/confirmation.svelte';
  import Collapsible from './lib/ui/Collapsible.svelte';
  import TabsList from './lib/ui/TabsList.svelte';
  import TabsTrigger from './lib/ui/TabsTrigger.svelte';
  import { Tabs } from 'bits-ui';
  import Card from './lib/ui/Card.svelte';
  import Checkbox from './lib/ui/Checkbox.svelte';
  import Input from './lib/ui/Input.svelte';
  import Label from './lib/ui/Label.svelte';
  import Textarea from './lib/ui/Textarea.svelte';

  import { onMount, tick, untrack } from 'svelte';

  import Modal from './lib/Modal.svelte';
  import AddRepository from './lib/AddRepository.svelte';
  import AppUpdates from './lib/AppUpdates.svelte';
  import { version as appVersion } from '../package.json';
  import WorkspaceFolder from './lib/WorkspaceFolder.svelte';
  import Branches from './lib/Branches.svelte';
  import StartupSettings from './lib/StartupSettings.svelte';
  import {launchFetch} from './lib/launch-fetch';
  import WorkflowSettings from './lib/WorkflowSettings.svelte';
  import QuickSwitch from './lib/QuickSwitch.svelte';
  import LocateRepo from './lib/LocateRepo.svelte';
  import {draftKey,readDraft,saveDraft,forgetRepositoryDrafts,type DraftContext} from './lib/draft-store';
  import {GitFailure,type AppPreferences,type RepoWorkflow,type RepoPreferences,type RepoContext,type GitExpectation} from './lib/api';
  import ImportRepositories from './lib/ImportRepositories.svelte';
  import Walkthrough from './lib/Walkthrough.svelte';
  import LiveLogs from './lib/LiveLogs.svelte';
  import PlatformIcon from './lib/PlatformIcon.svelte';
  import { remoteInfo } from './lib/platform';
  import TokenInput from './lib/TokenInput.svelte';
  import CommitHistory from './lib/CommitHistory.svelte';
  import ConflictEditor from './lib/ConflictEditor.svelte';
  import DiffView from './lib/DiffView.svelte';
  import PullRequests from './lib/PullRequests.svelte';

  import { Activity, ArrowDown, ArrowLeftRight, ArrowUp, Check, CircleAlert, Clock3, Code2, FileCode, Folder, FolderGit2, GitBranch, GitCommitHorizontal, Github, History, LayoutDashboard, LoaderCircle, Pin, Plus, RefreshCw, Search, Settings, ShieldCheck, Tags, X } from 'lucide-svelte';

  import { api, filteredRepos, type Repo, type Activity as ActivityItem, type Account, type RepoDetails, type LoginCode, type WorkspaceSettings, type Discovery, type RepoSummary } from './lib/api';

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
  let showBranches = $state(false);
  let showStartup = $state(false), startupBusy = $state(false);
  let launchFailures = $state<Record<number,string>>({}), retryingLaunchFetch = $state(false);
  let launchBatch = Date.now();
  let metadata=$state(new Map<number,RepoWorkflow>());
  let repoMetadataReady=$state(false),workflowReady=$state(false);
  let workflowApp=$state<AppPreferences>({save_drafts:true,warn_default:true,editor:null});
  let showQuick=$state(false),showRepoSettings=$state(false),locating=$state<Repo|null>(null);
  let repoContext=$state<RepoContext|null>(null),recentBranches=$state<string[]>([]);
  let defaultCommit=$state<{repo:Repo;branch:string;head:string|null;message:string;files:string[]}|null>(null);
  let selectionMode=$state<'include'|'exclude'>('include');
  let markedFiles=$state(new Set<string>());
  let branchNameForCommit=$state(''),undoConfirm=$state<{repo:Repo; branch:string; head:string}|null>(null),recoveredMessage=$state('');
  let showFileSearch=$state(false),showFolded=$state(false);
  let fileSearch=$state<HTMLInputElement>();
  let gitFailure=$state<{code:string;branch:string|null}|null>(null);
  let showWalkthrough = $state(false);
  let showLiveLogs = $state(false);
  let workspace=$state<WorkspaceSettings>({default_folder:null,auto_detect:false,setup_complete:false});
  let showSetup=$state(false),setupBusy=$state(false),showImport=$state(false);
  let found=$state<Discovery|null>(null),importInitial=$state<Discovery|null>(null);
  let discoverySequence=0;let detectionBusy=false;let discoveryError=$state('');
  async function scanDefault(){if(detectionBusy||!workspace.auto_detect||!workspace.default_folder)return;detectionBusy=true;const sequence=++discoverySequence;try{const result=await api.discover('default_folder');if(sequence===discoverySequence){found=result;discoveryError='';}}catch(e){if(sequence===discoverySequence)discoveryError=String(e);}finally{detectionBusy=false;}}
  function workspaceSaved(settings:WorkspaceSettings){workspace=settings;showSetup=false;showStartup=true;++discoverySequence;found=null;discoveryError='';void scanDefault();}
  async function finishSetup(){showStartup=false;try{if(!await api.walkthroughSeen()&&alive)showWalkthrough=true;}catch(e){if(alive)error=String(e);}}
  async function skipSetup(){if(setupBusy)return;setupBusy=true;try{await workspaceSaved(await api.saveWorkspaceSettings({...workspace,setup_complete:true}));}catch(e){error=String(e);}finally{setupBusy=false;}}
  function openImport(initial:Discovery|null=null){importInitial=initial;showImport=true;}
  async function imported(count:number){showImport=false;found=null;query='';view='all';selected=null;notice=`Imported ${count} repositories`;await load();}


  let showAccount = $state(false);

  let showOrganize = $state(false);let organizing=$state(false);let pendingPins=$state<number[]>([]);
  let alive=true,metadataRevision=0;const localRevisions=new Map<number,number>();const statusVersions=new Map<number,number>();const statusQueue=new Set<number>();const activeStatuses=new Set<number>();
  function changedMetadata(id:number){localRevisions.set(id,++metadataRevision);}
  function requestStatuses(ids:number[]){for(const id of ids)if(!activeStatuses.has(id))statusQueue.add(id);drainStatuses();}
  function applySummary(summary:RepoSummary){repos=repos.map(repo=>{
    if(repo.id!==summary.id || repo.path!==summary.path || repo.registration!==summary.registration)return repo;
    const counts=summary.status_error ? (!repo.branch ? workflowFor(repo)?.cached_status??{} : {}) : {branch:summary.branch,dirty:summary.dirty,ahead:summary.ahead,behind:summary.behind,conflicts:summary.conflicts};
    return {...repo,...counts,missing:summary.missing,status_error:summary.status_error,status_pending:false};
  });}
  function drainStatuses(){if(!alive)return;while(activeStatuses.size<2){const id=[...statusQueue].find(id=>!(working&&id===selected));if(id===undefined)return;statusQueue.delete(id);if(!repos.some(r=>r.id===id))continue;activeStatuses.add(id);const version=statusVersions.get(id)??0;void api.summary(id).then(summary=>{if(alive&&(statusVersions.get(id)??0)===version)applySummary(summary);}).catch(e=>{if(alive&&(statusVersions.get(id)??0)===version)repos=repos.map(r=>r.id===id?{...r,status_pending:false,status_error:String(e)}:r);}).finally(()=>{activeStatuses.delete(id);drainStatuses();});}}
  async function togglePin(repo:Repo){if(pendingPins.includes(repo.id))return;const value=!repo.pinned;changedMetadata(repo.id);pendingPins=[...pendingPins,repo.id];repos=repos.map(r=>r.id===repo.id?{...r,pinned:value}:r);try{await api.pin(repo.id,value);await loadMetadata();}catch(e){if(alive){repos=repos.map(r=>r.id===repo.id?{...r,pinned:!value}:r);error=String(e);}}finally{changedMetadata(repo.id);pendingPins=pendingPins.filter(id=>id!==repo.id);}}


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
  let pendingGit = $state<{id:number;action:string;args:string[];title:string;description:string;expected:GitExpectation}|null>(null);

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

  let loadFlight: Promise<void> | null = null;

  let selectionSequence = 0;

  let searchInput = $state<HTMLInputElement>();

  let page = $state(1);

  const pageSize = 50;

  $effect(() => { query; view; page = 1; });
  const commitDraft=$derived.by(():DraftContext|null=>{
    if(!current||!gitState||gitState.branch==='(detached)')return null;
    return {target:targetOf(current),scope:{branch:gitState.branch,account:'',base:''}};
  });
  const commitDraftKey=$derived(commitDraft&&draftKey(commitDraft));
  let draftLoading=$state(false),draftSequence=0;
  const messageTooLong=$derived(message.length>500);
  $effect(()=>{
    const key=commitDraftKey;
    if(key)void untrack(()=>restoreCommitDraft(key));
    else {++draftSequence;message='';draftLoading=false;}
  });

  const current = $derived(repos.find(r => r.id === selected) ?? null);
  const gitState = $derived(details?.git??null);
  const pushBlocked = $derived(working||!gitState?.has_head||gitState.dirty>0||gitState.operation!==null||gitState.conflicts.length>0||!gitState.remote||gitState.upstream_gone);

  const visible = $derived(orderRepos(view==='attention'
    ? filteredRepos(repos,query,'all').filter(r=>r.status_error||r.dirty||r.ahead||r.behind||launchFailures[r.id]||r.ci_conclusion==='failure')
    : filteredRepos(repos, query, view)));

  const fileNeedle = $derived(fileQuery.toLocaleLowerCase());
  const activeSettings=$derived(current ? workflowFor(current) : undefined);
  const reviewPaths=$derived(activeSettings?.review_paths??[]);
  const changedFiles=$derived([...new Map((details?.changes ?? []).map(file=>[file.path,file])).values()]);
  const markedCount=$derived(changedFiles.filter(file=>markedFiles.has(file.path)).length);
  const commitFiles=$derived(changedFiles.filter(file=>selectionMode==='exclude'
    ? !markedFiles.has(file.path)
    : markedFiles.size===0||markedFiles.has(file.path)).map(file=>file.path));
  const selectionBranch=$derived(gitState?.branch);
  $effect(()=>{selectionBranch;markedFiles=new Set();});
  function markFile(path:string,checked:boolean){
    const next=new Set(markedFiles);
    if(checked)next.add(path);else next.delete(path);
    markedFiles=next;
  }
  const matchingFiles=$derived(changedFiles.filter(file => file.path.toLocaleLowerCase().includes(fileNeedle)));
  const foldedFiles=$derived(matchingFiles.filter(file=>!gitState?.conflicts.includes(file.path) && reviewPaths.some(path=>file.path===path || file.path.startsWith(path.endsWith('/')?path:path+'/'))));
  const fileMatches=$derived(showFolded||fileNeedle ? matchingFiles : matchingFiles.filter(file=>!foldedFiles.includes(file)));
  const repoActivity = $derived(current ? activities.filter(item => item.repo_id === current.id).slice(0, 3) : []);
  const attention = $derived(repos.filter(r => r.status_error || r.dirty || r.ahead || r.behind || launchFailures[r.id] || r.ci_conclusion === 'failure').length);
  function targetOf(repo:Repo){return {id:repo.id,path:repo.path,registration:repo.registration};}
  function workflowFor(repo:Repo){const entry=metadata.get(repo.id);return entry?.path===repo.path&&entry.registration===repo.registration?entry:undefined;}
  function orderRepos(rows:Repo[]){
    return rows.slice().sort((a,b)=>Number(b.pinned)-Number(a.pinned) || (a.pinned
      ? (workflowFor(a)?.pin_position??Number.MAX_SAFE_INTEGER)-(workflowFor(b)?.pin_position??Number.MAX_SAFE_INTEGER)
      : a.name.localeCompare(b.name) || a.id-b.id));
  }
  let workflowRead=0;
  async function loadMetadata(){
    const request=++workflowRead;
    try {const rows=await api.repoWorkflow();if(alive&&request===workflowRead){
      metadata=new Map(rows.map(row=>[row.id,row]));repoMetadataReady=true;
      repos=repos.map(repo=>{
        const cached=workflowFor(repo)?.cached_status;
        return !repo.branch&&cached?{...repo,...cached}:repo;
      });
    }}
    catch(reason){if(alive&&request===workflowRead)error=String(reason);}
  }
  async function loadWorkflowApp(){
    try{const value=await api.workflowApp();if(alive){workflowApp=value;workflowReady=true;}}
    catch(reason){if(alive)error=String(reason);}
  }
  async function restoreCommitDraft(key:string){
    const context=commitDraft;
    if(!context)return;
    const revision=++draftSequence;
    draftLoading=true;message='';recoveredMessage='';
    try{
      const value=await readDraft(context,workflowApp.save_drafts);
      if(revision===draftSequence&&key===commitDraftKey)message=value?.title??'';
    }catch(reason){if(revision===draftSequence)error=String(reason);}
    finally{if(revision===draftSequence)draftLoading=false;}
  }
  function updateDraft(value:string){
    message=value;
    if(commitDraft)void saveDraft(commitDraft,{title:value,body:''},workflowApp.save_drafts).catch(reason=>{error=String(reason);});
  }
  async function visit(repo:Repo, branch:string|null){
    try{
      const visited=await api.visitRepo(targetOf(repo),branch==='(detached)'?null:branch);
      if(current?.id===repo.id&&current.path===repo.path)recentBranches=visited;
      await loadMetadata();
    }catch(reason){if(current?.id===repo.id)error=String(reason);}
  }
  async function readRepoContext(repo:Repo){
    try{
      const result=await api.repoContext(targetOf(repo));
      if(current?.id===repo.id&&current.path===repo.path)repoContext=result;
      return result;
    }catch(reason){if(current?.id===repo.id)error=String(reason);return null;}
  }
  async function openLocation(repo:Repo,action:'editor'|'terminal'|'folder'|'worktree',reference:string|null=null){
    try{await api.openRepoLocation(targetOf(repo),action,reference);}
    catch(reason){error=String(reason);}
  }
  async function arrangePin(repo:Repo,earlier:boolean){
    try{await api.movePin(targetOf(repo),earlier);await loadMetadata();}
    catch(reason){error=String(reason);}
  }

  const pinned = $derived(repos.filter(r => r.pinned).length);

  const collections = $derived([...new Set(repos.map(r => r.collection).filter((x): x is string => !!x))]);

  const date = (sec:number) => new Date(sec*1000).toLocaleString(undefined,{month:'short',day:'numeric',hour:'2-digit',minute:'2-digit'});

  const short = (value: string, max = 45) => value.length > max ? '…' + value.slice(-(max-1)) : value;

  let supplementaryFlight:Promise<void>|null=null;
  function loadSupplementary(){if(supplementaryFlight)return;supplementaryFlight=Promise.allSettled([api.activity().then(rows=>{if(alive)activities=rows;}),api.accounts().then(rows=>{if(alive)accounts=rows;})]).then(results=>{const failure=results.find(r=>r.status==='rejected');if(alive&&failure?.status==='rejected')error=String(failure.reason);}).finally(()=>supplementaryFlight=null);}
  async function load() {
    loadSupplementary();
    if(loadFlight)return loadFlight;
    const revision=metadataRevision;
    loadFlight=api.repos().then(rows=>{if(!alive)return;const previous=new Map(repos.map(r=>[r.id,r]));repos=rows.map(row=>{
      const old=previous.get(row.id);
      if(!old||old.path!==row.path||old.registration!==row.registration){
        const cached=workflowFor(row)?.cached_status;
        return cached?{...row,...cached}:row;
      }
      const git={branch:old.branch,dirty:old.dirty,ahead:old.ahead,behind:old.behind,conflicts:old.conflicts,missing:old.missing,status_error:old.status_error,status_pending:old.status_pending};
      const local=(localRevisions.get(row.id)??0)>revision||pendingPins.includes(row.id)?{pinned:old.pinned,collection:old.collection,tags:old.tags}:{};
      return {...row,...git,...local};
    });requestStatuses(repos.map(r=>r.id));}).catch(e=>{if(alive)error=String(e);}).finally(()=>{if(alive)loading=false;loadFlight=null;});
    return loadFlight;
  }

  function beginSelection(id: number) {
    const sequence = ++selectionSequence;
    selected = id; message = ''; newBranch = ''; details = null; detailError = ''; activeTab = 'changes'; actionResult = '';
    repoContext=null;recentBranches=[];recoveredMessage='';gitFailure=null;showFolded=false;showFileSearch=false;markedFiles=new Set();
    fileQuery = ''; selectedFile = null; diffCode = ''; diffError = ''; diffLoading = false; ++diffSequence;
    return sequence;
  }
  async function selectRepo(id:number) {
    const sequence = beginSelection(id);
    const repo=repos.find(repo=>repo.id===id);
    if(repo)void readRepoContext(repo);
    try { const result = await api.details(id); if (sequence === selectionSequence && selected === id) { details = result; updateRepoStatus(id,result); if(repo)void visit(repo,result.git.branch); if (result.changes[0]) void showDiff(result.changes[0].path, id); } }

    catch(e) { if (sequence === selectionSequence) detailError = String(e); }

  }

  async function act(fn:()=>Promise<unknown>,msg:string,refreshWorkspace=true) {

    if (working) return false;

    const actionRepo = selected;
    working=true; error='';if(actionRepo!==null)statusVersions.set(actionRepo,(statusVersions.get(actionRepo)??0)+1);

    try {

      const result=await fn(); notice=msg; if (selected === actionRepo) actionResult=typeof result==='string'?result:'';

      if(refreshWorkspace)await load();else void api.activity().then(rows=>activities=rows).catch(()=>{});

      const id=selected;

      if(id!==null) { try { const result=await api.details(id); if(selected===id){details=result;updateRepoStatus(id,result);const file=result.changes.some(file=>file.path===selectedFile)?selectedFile:result.changes[0]?.path??null;void showDiff(file,id);} } catch(e) {detailError=String(e);} }

      return true;

    } catch(e) {error=String(e);if(refreshWorkspace)await load();const id=selected;if(id!==null){try{const result=await api.details(id);if(selected===id){details=result;updateRepoStatus(id,result);}}catch{}}return false;} finally {working=false;drainStatuses();}

  }

  async function repositoryAdded(repo:Repo,warning?:string|null){showAdd=false;notice='Repository ready';error=warning??'';await load();if(!repos.some(r=>r.id===repo.id))repos=[...repos,repo];await selectRepo(repo.id);}
  function openAdd(){showAdd=true;}
  function closeAdd(){showAdd=false;}
  async function prepareTour(target:string){
    if(target==='commit'&&repos[0]){await selectRepo(repos[0].id);activeTab='changes';}
    else{selected=null;view=target==='connect'?'settings':'all';}
    await tick();
  }
  function updateRepoStatus(id:number,result:RepoDetails){statusVersions.set(id,(statusVersions.get(id)??0)+1);repos=repos.map(repo=>repo.id===id?{...repo,branch:result.git.branch,dirty:result.git.dirty,ahead:result.git.ahead,behind:result.git.behind,conflicts:result.git.conflicts.length,missing:false,status_error:null,status_pending:false}:repo);}
  async function refresh(){
    if(refreshing||working)return;refreshing=true;error='';const sequence=selectionSequence;
    try{const id=selected;if(id===null)await load();if(id!==null){const result=await api.details(id);if(selected===id&&sequence===selectionSequence){details=result;updateRepoStatus(id,result);detailError='';if(activeTab==='changes'){const file=result.changes.some(item=>item.path===selectedFile)?selectedFile:result.changes[0]?.path??null;void showDiff(file,id);}}}}
    catch(e){detailError=String(e);}finally{refreshing=false;}
  }

  async function fetchOnLaunch() {
    try {
      const settings = await api.fetchSettings();
      if (!settings.enabled || !alive) return;
      await load();
      await tick();
      await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      if (!alive) return;
      await loadMetadata();
      const batch=++launchBatch;
      await launchFetch(repos.map(repo=>({...repo,fetch_mode:workflowFor(repo)?.fetch_mode})), settings.scope, id=>launchRepo(id,batch));
      await loadMetadata();
    } catch (reason) { if (alive) error = String(reason); }
  }

  async function launchRepo(id:number,batch:number){
    if(!alive)return;
    try{
      if(await api.launchFetch(id,batch))requestStatuses([id]);
      delete launchFailures[id];
    }catch(reason){launchFailures[id]=String(reason);}
  }
  async function retryLaunchFetch(repository?:number) {
    if (retryingLaunchFetch) return;
    retryingLaunchFetch = true;
    try {
      const batch=++launchBatch;
      await launchFetch(repos.filter(repo=>launchFailures[repo.id]&&(repository===undefined||repo.id===repository)),'all',id=>launchRepo(id,batch));
      await loadMetadata();
    } finally { retryingLaunchFetch = false; }
  }

  async function openOrganization(repo: Repo | undefined = current ?? repos[0]) {
    if(working)return;
    if(!repo){openAdd();return;}
    if(selected!==repo.id)void selectRepo(repo.id);
    if(working||selected!==repo.id)return;
    collection=repo.collection?[repo.collection]:[];tags=[...repo.tags];showOrganize=true;
  }
  async function organize(){if(!current||organizing)return;const id=current.id;const group=collection[0]??'';const values=[...tags];organizing=true;try{await api.organize(id,group,values);if(alive){changedMetadata(id);repos=repos.map(r=>r.id===id?{...r,collection:group||null,tags:values}:r);showOrganize=false;notice='Collection updated';}}catch(e){if(alive)error=String(e);}finally{organizing=false;}}

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

  async function runGit(action:string,args:string[]=[]){
    if(!current||working)return false;
    const repo=current,expected=gitState?{branch:gitState.branch,head:gitState.head,path:current.path,registration:current.registration,author:action==='commit_paths'?repoContext?.author??undefined:undefined}:null;
    gitFailure=null;
    const ok=await act(async()=>{
      try{return await api.git(repo.id,action,args,expected);}
      catch(reason){if(reason instanceof GitFailure)gitFailure={code:reason.code,branch:action==='switch_branch'?args[0]:null};throw reason;}
    },action==='commit_paths'?'Commit created':`Git ${action.replaceAll('_',' ')} completed`,false);
    if(['fetch','pull','merge_upstream'].includes(action))void loadMetadata();
    if(ok&&action==='fetch')delete launchFailures[repo.id];
    if(ok&&['switch_branch','create_branch'].includes(action)&&current?.id===repo.id){
      void visit(repo,gitState?.branch??null);
      void readRepoContext(repo);
    }
    return ok;
  }

  async function finishCommit(text:string,files:string[]){
    const context=commitDraft,key=commitDraftKey,sequence=selectionSequence;
    if(await runGit('commit_paths',[text,...files])){
      if(context&&(commitDraftKey!==key||message===text))await saveDraft(context,null,workflowApp.save_drafts).catch(reason=>{error=String(reason);});
      if(commitDraftKey===key&&message===text)message='';
      if(selectionSequence===sequence)markedFiles=new Set();
    }
  }
  function commit(){
    if(!current||!gitState||working||draftLoading||messageTooLong||!message.trim()||!commitFiles.length)return;
    const request={repo:current,branch:gitState.branch,head:gitState.head,message,files:[...commitFiles]};
    const sequence=selectionSequence;
    const lastAuthor=repoContext?.author;
    void readRepoContext(current).then(context=>{
      if(!context||sequence!==selectionSequence||current?.path!==request.repo.path||current.registration!==request.repo.registration||gitState?.branch!==request.branch||gitState.head!==request.head||working)return;
      if(request.message!==message||request.files.length!==commitFiles.length||request.files.some((file,index)=>file!==commitFiles[index])){
        error='The commit selection or message changed. Review it and click Commit again.';return;
      }
      if(!context.author){error='Git needs a commit author. Open Repository settings to set a name and email.';return;}
      if(lastAuthor!==context.author){error='Review the current Git author before committing.';return;}
      const override=activeSettings?.warn_default;
      if(context.default_branch===request.branch&&(override==='on'||(override!=='off'&&workflowApp.warn_default))){
        branchNameForCommit='';defaultCommit=request;
      }else void finishCommit(request.message,request.files);
    });
  }
  async function acceptDefaultCommit(create:boolean){
    const task=defaultCommit;
    if(!task||task.repo.id!==current?.id||task.repo.registration!==current.registration||task.repo.path!==current.path||task.branch!==gitState?.branch||task.head!==gitState.head)return;
    defaultCommit=null;
    if(!create){await finishCommit(task.message,task.files);return;}
    if(await runGit('create_branch',[branchNameForCommit])&&current?.id===task.repo.id){
      await tick();
      if(commitDraftKey)await restoreCommitDraft(commitDraftKey);
      if(message&&message!==task.message)recoveredMessage=task.message;
      else updateDraft(task.message);
    }
  }
  async function undoCommit(){
    if(!undoConfirm||working)return;
    const {repo,branch,head}=undoConfirm;
    const expected={branch,head};
    undoConfirm=null;
    let text='';
    if(await act(async()=>{text=await api.undoCommit(targetOf(repo),expected);},'Commit undone. Files remain staged.',false)
      &&current?.id===repo.id&&gitState?.branch===expected.branch){
      if(message&&message!==text || text.length>500)recoveredMessage=text;
      else updateDraft(text);
      activeTab='changes';
    }
  }

  async function createBranch(){const id=selected;const name=newBranch;if(await runGit('create_branch',[name])&&selected===id&&newBranch===name)newBranch='';}
  function revertCommit(hash:string,merge:boolean){queueGit('revert','Revert commit?', 'Create a new commit that undoes '+hash.slice(0,8)+(merge?' relative to its first parent':'')+'. Existing history will be preserved.',[hash]);}
  function queueGit(action:string,title:string,description:string,args:string[]=[]){if(!current||!gitState||working)return;pendingGit={id:current.id,action,args,title,description,expected:{branch:gitState.branch,head:gitState.head,path:current.path,registration:current.registration}};}
  async function executePendingGit(){
    if(!pendingGit)return;
    const task=pendingGit;pendingGit=null;gitFailure=null;
    await act(async()=>{
      try{return await api.git(task.id,task.action,task.args,task.expected);}
      catch(reason){if(reason instanceof GitFailure)gitFailure={code:reason.code,branch:null};throw reason;}
    },task.action==='revert'?'Revert commit created':'Git action completed',false);
    if(['pull','merge_upstream'].includes(task.action))void loadMetadata();
  }

  async function removeRepo(repo: Repo) {
    const id = repo.id;
    if(!await confirm({title:`Remove ${repo.name} from Reponiq?`,description:'The repository files stay in their folder.',action:'Remove from workspace',destructive:true}))return;
    await act(async()=>{await api.remove(id);forgetRepositoryDrafts(repo);delete launchFailures[id];if(selected===id){selected=null;details=null;message='';diffLoading=false;++diffSequence;}},'Repository removed');
  }
  async function disconnectAccount(account:Account){
    if(await confirm({title:`Disconnect @${account.login}?`,description:'Reponiq will delete the saved token from this device.',action:'Disconnect',destructive:true})){
      await act(()=>api.disconnect(account.id),'Account disconnected');
    }
  }
  async function useRecoveredMessage(){
    const text=recoveredMessage,key=commitDraftKey;
    if(message&&!await confirm({title:'Replace the current draft?',action:'Use recovered message'}))return;
    if(key===commitDraftKey&&text===recoveredMessage){updateDraft(text);recoveredMessage='';}
  }
  async function showDiff(file: string | null, id = selected) {
    if (id === null) return;
    const sequence = ++diffSequence;
    selectedFile = file; diffCode = ''; diffError = ''; diffLoading = true;
    try { const code = await api.git(id, 'diff', file === null ? [] : [file]); if (sequence === diffSequence && selected === id) diffCode = code; }
    catch(e) { if(sequence === diffSequence && selected === id) diffError=String(e); }
    finally { if(sequence === diffSequence)diffLoading=false; }
  }
  function modalOpen(){
    return document.querySelector('[data-slot="dialog-content"][aria-modal="true"]') !== null;
  }

  async function keyboard(event: KeyboardEvent){

    const editing=event.target instanceof HTMLInputElement||event.target instanceof HTMLTextAreaElement;
    if(event.defaultPrevented||event.isComposing||modalOpen())return;

    if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='k'){event.preventDefault();if(!working)showQuick=true;}
    if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='f'&&current&&activeTab==='changes'){event.preventDefault();showFileSearch=true;await tick();fileSearch?.focus();}

    if(event.key==='Escape'&&!editing&&!showAdd&&!showAccount&&!showOrganize&&!showBranch&&!conflictFile){selected=null;details=null;}

  }

  function confirmGit(action:string){const labels:{[k:string]:[string,string]}={pull:['Pull changes?','Fetch and fast-forward this branch from its upstream.'],push:['Push branch?','Push the committed branch to its tracking remote.'],merge_upstream:['Merge upstream?','Combine the latest upstream with this branch without rewriting history. Conflicts may need to be resolved.'],abort_operation:['Abort Git operation?','Resolution work from this operation may be discarded. Git will attempt to restore the pre-operation state.'],unstage_all:['Unstage all files?','Keep the working files and remove them from the next commit.']};const label=labels[action];if(label)queueGit(action,...label);else void runGit(action);}

  onMount(()=>{
    void load();
    void loadMetadata();
    void loadWorkflowApp();
    void fetchOnLaunch();
    void api.workspaceSettings().then(async settings=>{if(!alive)return;workspace=settings;if(!settings.setup_complete)showSetup=true;else{if(!await api.walkthroughSeen()&&alive)showWalkthrough=true;void scanDefault();}}).catch(e=>{if(alive)error=String(e);});
    void api.browserLoginAvailable().then(value=>{if(alive)browserAvailable=value;}).catch(()=>browserAvailable=false);
    const idle=()=>document.visibilityState==='visible'&&!working&&!modalOpen();
    const timer=setInterval(()=>{if(idle()){void load();void scanDefault();}},60000);
    const statusTimer=setInterval(()=>{if(idle()){if(selected!==null)void refresh();else requestStatuses(visible.slice((page-1)*pageSize,page*pageSize).map(r=>r.id));}},10000);
    let lastFocus=0;const focus=()=>{if(idle()&&Date.now()-lastFocus>2000){lastFocus=Date.now();if(selected!==null)void refresh();else void load();}};
    window.addEventListener('focus',focus);document.addEventListener('visibilitychange',focus);
    return()=>{alive=false;statusQueue.clear();clearInterval(timer);clearInterval(statusTimer);window.removeEventListener('focus',focus);document.removeEventListener('visibilitychange',focus);closeAccount();};
  });

</script>



<svelte:window onkeydown={keyboard}/>

<svelte:head><title>Reponiq — Repository workspace</title><meta name="description" content="A local-first Git and GitHub dashboard"/></svelte:head>

<a class="skip-link" href="#main-content">Skip to workspace</a>

<div class="app-shell">

  <aside class="sidebar">

    <div class="brand"><div class="brand-mark"><svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M6 19V5h7a4 4 0 0 1 0 8H6m7 0 5 6" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/><circle cx="18" cy="19" r="2" fill="currentColor" stroke="none"/></svg></div><div><div class="brand-title">Reponiq</div></div></div>

    <div class="side-section">WORKSPACE</div>

    <Button title="Overview" class={[{"active": view==='all' && selected===null}]} onclick={()=>{selected=null;view='all'}}><LayoutDashboard size={17}/> Overview</Button>

    <Button title="Pinned repositories" class={[{"active": view==='pinned' && selected===null}]} onclick={()=>{selected=null;view='pinned'}}><Pin size={17}/> Pinned <span class="side-count">{pinned}</span></Button>

    <Button title="Needs attention" class={[{"active": view==='attention' && selected===null}]} onclick={()=>{selected=null;view='attention'}}><CircleAlert size={17}/> Needs attention <span class="side-count">{attention}</span></Button>

    <Button title="Activity history" class={[{"active": view==='activity'}]} onclick={()=>{selected=null;view='activity'}}><History size={17}/> Activity history</Button>

    <div class="side-head"><span class="side-section">COLLECTIONS</span><Button class="icon-only" title="Add collection when organizing a repository" disabled={working} onclick={()=>openOrganization()}><Plus size={15}/></Button></div>

    {#each collections as c}<Button class={[{"active": view==='collection:'+c}]} onclick={()=>{selected=null;view='collection:'+c}}><Folder size={17}/>{c}</Button>{/each}

    {#if collections.length===0}<div class="muted-sidebar">Organize repositories into groups</div>{/if}

    <div class="sidebar-grow"></div>

    <div class="sidebar-bottom">

      <Button title="Settings" onclick={()=>{selected=null;view='settings'}} class={[{"active": view==='settings'}]}><Settings size={17}/> Settings</Button>

      <AppUpdates busy={working}/>
      <div class="connection"><span class="live-dot"></span> Local workspace <span class="version">v{appVersion}</span></div>

    </div>

  </aside>

  <div class="main-area">


    <main class="content" class:workbench-content={!!current} id="main-content" tabindex="-1" aria-busy={loading||working}>

      {#if error}<div class="alert error" role="alert"><CircleAlert size={16}/><span>{error}</span>
        {#if gitFailure?.code==='upstream'&&details}<Button variant="secondary" onclick={()=>showBranches=true}>Choose branch</Button>
        {:else if gitFailure?.code==='missing'&&current}<Button variant="secondary" onclick={()=>locating=current}>Locate folder</Button>
        {:else if gitFailure?.code==='conflict'&&current}<Button variant="secondary" onclick={()=>activeTab='changes'}>Review changes</Button>
        {:else if gitFailure?.code==='stale'&&current}<Button variant="secondary" onclick={()=>refresh()}>Refresh repository</Button>{/if}
        <Button title="Dismiss error" onclick={()=>{error='';gitFailure=null;}}><X size={15}/></Button></div>{/if}

      {#if notice}<div class="alert success" role="status"><Check size={16}/><span>{notice}</span><Button title="Dismiss notification" onclick={()=>notice=''}><X size={15}/></Button></div>{/if}
      {#if Object.keys(launchFailures).length}<div class="alert error" role="status"><CircleAlert size={16}/><span>{Object.keys(launchFailures).length} repositories need help fetching.</span><Button variant="secondary" disabled={working} onclick={()=>{selected=null;details=null;view='attention';query='';}}>View affected repositories</Button></div>{/if}

      {#if current}
        <div class="workbench-shell">

          <section class="workbench-main" aria-label="Git workbench">
            <div class="page-heading workbench-heading">
              <div><Button class="back-link" onclick={()=>{selected=null;details=null}}><ArrowLeftRight size={14}/> All repositories</Button><div class="heading-row"><PlatformIcon remote={current.remote_url} size={25}/><h1>{current.name}</h1></div><p title={current.path}>{current.path}</p></div>
              <div class="heading-actions"><Button variant="secondary" aria-label={current.pinned?'Unpin repository':'Pin repository'} disabled={pendingPins.includes(current.id)} onclick={()=>togglePin(current)}><Pin size={16}/></Button><Button variant="secondary" disabled={working} onclick={()=>openOrganization(current)}><Tags size={15}/> Organize</Button></div>
            </div>
            <div class="workbench-tools">
              <Button class="secondary current-branch" aria-label={'Switch branch. Current branch: '+current.branch} disabled={working||!details} onclick={()=>showBranches=true}><GitBranch size={16}/><span>{current.branch}</span></Button>
              <div class="toolbar"><Button onclick={()=>runGit('fetch')} disabled={working||!!current.status_error||!gitState?.remotes.length}><RefreshCw size={14}/> Fetch</Button><Button onclick={()=>confirmGit('pull')} title={gitState?.upstream??'Publish this branch to set an upstream'} disabled={working||!!current.status_error||!gitState?.upstream||!!gitState?.dirty||!!gitState?.operation}><ArrowDown size={14}/> Pull</Button><Button variant="primary" onclick={()=>confirmGit('push')} title={gitState?.dirty?'Commit all changes before pushing':!gitState?.remote?'No tracking remote configured':'Push committed changes'} disabled={pushBlocked||!!current.status_error}><ArrowUp size={14}/> {gitState?.upstream?'Push':'Publish branch'}</Button></div>
            </div>
            <Collapsible class="repo-actions">
              {#snippet heading()}Repository actions{/snippet}
              <div class="toolbar">
              <Button variant="secondary" disabled={!repoMetadataReady||!workflowReady} onclick={()=>showRepoSettings=true}>Repository settings</Button>
              <Button variant="secondary" onclick={()=>openLocation(current!,'editor')}>Open in editor</Button>
              <Button variant="secondary" onclick={()=>openLocation(current!,'terminal')}>Open terminal</Button>
              <Button variant="secondary" onclick={()=>openLocation(current!,'folder')}>Open folder</Button>
              <Button variant="secondary" onclick={()=>locating=current}>Locate folder</Button>
              </div>
            </Collapsible>
            {#if launchFailures[current.id]}<div class="operation-banner" role="status"><p>{launchFailures[current.id]}</p><Button variant="secondary" disabled={retryingLaunchFetch} onclick={()=>retryLaunchFetch(current?.id)}>Retry fetch</Button></div>{/if}
            <div class="workbench-status"><span class="status-label" class:status-warning={current.dirty>0||current.behind>0||!!current.status_error}><i></i>{current.status_error?'Unavailable':current.dirty?'Changed':'Clean'}</span><span><ArrowUp size={13}/>{current.ahead} ahead</span><span><ArrowDown size={13}/>{current.behind} behind</span>{#if current.ci_conclusion||current.ci_status}<span class="workbench-ci"><PlatformIcon remote="https://github.com" size={14}/>{current.ci_conclusion??current.ci_status}</span>{/if}</div>
            {#if gitState?.operation||gitState?.conflicts.length}<section class="operation-banner" aria-label="Git operation"><div><strong>{gitState.operation ? gitState.operation+' paused' : 'Resolve conflicting files'}</strong><span>{gitState.conflicts.length?gitState.conflicts.length+' conflicting files':'Conflicts resolved. Continue to finish.'}</span></div>{#if gitState.operation}<div class="toolbar"><Button variant="primary" disabled={working||gitState.conflicts.length>0} onclick={()=>runGit('continue_operation')}>Continue</Button><Button class="secondary dangerous" disabled={working} onclick={()=>confirmGit('abort_operation')}>Abort</Button></div>{/if}</section>{:else if gitState&&gitState.ahead>0&&gitState.behind>0}<section class="operation-banner"><div><strong>Branches have diverged</strong><span>{gitState.ahead} local · {gitState.behind} upstream commits</span></div><Button variant="secondary" disabled={working||gitState.dirty>0} onclick={()=>confirmGit('merge_upstream')}>Merge upstream</Button></section>{/if}
            {#if gitState && !gitState.operation && !gitState.conflicts.length}
              <div class="workflow-next" aria-label="Next step">
                {#if gitState.dirty}
                  <p>Review your changes, then choose what to commit.</p>
                  {#if activeTab!=='changes'}<Button variant="secondary" onclick={()=>activeTab='changes'}>Review changes</Button>{/if}
                {:else if !gitState.has_head}
                  <p>Add files and create your first commit.</p>
                {:else if gitState.upstream_gone}
                  <p>The upstream branch was deleted. Choose a different branch or configure tracking in Git.</p>
                {:else if !gitState.upstream && gitState.remote && gitState.branch!=='(detached)'}
                  <p>Publish this branch to share your commits.</p>
                {:else if gitState.ahead && !gitState.behind}
                  <p>Push your commits to update the remote branch.</p>
                {:else if gitState.behind && !gitState.ahead}
                  <p>Pull to update this branch with the latest remote commits.</p>
                {:else if current.github_full_name && gitState.upstream && !gitState.ahead && !gitState.behind && activeTab!=='pulls'}
                  <Button variant="secondary" onclick={()=>activeTab='pulls'}>Open pull requests</Button>
                {/if}
              </div>
            {/if}
            <Tabs.Root bind:value={() => activeTab, (tab) => {
              if (tab === 'changes' || tab === 'history' || tab === 'pulls') activeTab = tab;
            }}>
            <TabsList aria-label="Repository views">
              <TabsTrigger value="changes">Changes <span>{current.dirty}</span></TabsTrigger>
              <TabsTrigger value="history">Commit history</TabsTrigger>
              {#if current.github_full_name}<TabsTrigger value="pulls">Pull requests</TabsTrigger>{/if}
            </TabsList>
            {#if detailError}
              <div class="empty-block" role="alert"><CircleAlert size={20}/><span>{detailError}</span><Button variant="secondary" onclick={()=>locating=current}>Locate folder</Button><Button variant="secondary" onclick={()=>current&&selectRepo(current.id)}>Retry</Button></div>
            {:else if !details}<div class="empty-block"><LoaderCircle class="spin"/> Loading Git details...</div>
            {:else if activeTab==='changes'}
              <Tabs.Content value="changes">
              <div class="changes-workbench">
                <Card as="section" class="changed-files panel" aria-label="Changed files">
                  <div class="panel-title">Changed files <span>{changedFiles.length}</span></div>
                  {#if changedFiles.length}
                    <div class="commit-file-mode" role="group" aria-label="How to treat checked files">
                      <Button aria-pressed={selectionMode==='include'} disabled={working||!!gitState?.operation} onclick={()=>selectionMode='include'}>Include</Button>
                      <Button aria-pressed={selectionMode==='exclude'} disabled={working||!!gitState?.operation} onclick={()=>selectionMode='exclude'}>Exclude</Button>
                    </div>
                  {/if}
                  {#if showFileSearch||changedFiles.length>8}<div class="search-input file-search"><Search size={13}/><Input bind:ref={fileSearch} aria-label="Filter changed files" placeholder="Filter files" bind:value={fileQuery}/></div>{/if}
                  {#if foldedFiles.length&&!fileNeedle}<div class="panel-title">{foldedFiles.length} grouped files <Button class="text-action" onclick={()=>showFolded=!showFolded}>{showFolded?'Collapse group':'Show all'}</Button></div>{/if}
                  <div class="file-list">
                    {#each fileMatches.slice(0,200) as file (file.path)}
                      <div class="file-change-row"><Checkbox aria-label={(selectionMode==='include'?'Include ':'Exclude ')+file.path} checked={markedFiles.has(file.path)} disabled={working||!!gitState?.operation} onCheckedChange={checked=>markFile(file.path,checked)}/><Button class={["changed-file", {"file-selected": selectedFile===file.path}]} aria-pressed={selectedFile===file.path} title={file.path} onclick={()=>{if(selectedFile!==file.path)void showDiff(file.path,current.id);}}><FileCode size={15}/><span>{file.path}</span><b class:change-added={file.x==='A'||file.x==='?'} class:change-deleted={file.x==='D'||file.y==='D'} class:change-conflict={gitState?.conflicts.includes(file.path)}>{(file.x+file.y).trim()}</b></Button>{#if gitState?.conflicts.includes(file.path)}<Button class="resolve-file" onclick={()=>conflictFile=file.path}>Resolve</Button>{/if}</div>
                    {:else}<div class="empty-small"><Check size={17}/>{changedFiles.length?'No matching files':'Working tree clean'}</div>{/each}
                  </div>
                  {#if fileMatches.length>200}<small class="rail-limit">200 of {fileMatches.length} files. Filter to find a file.</small>{/if}
                </Card>
                <Card as="section" class="diff-panel panel" aria-label="File diff" aria-busy={diffLoading}>
                  <div class="panel-title"><span class="diff-filename" title={selectedFile??''}><FileCode size={15}/>{selectedFile??'Working tree'}</span><Button class="text-action" disabled={diffLoading} onclick={()=>showDiff(null,current.id)}>All changes</Button></div>
                  {#if diffLoading}<div class="diff-empty"><LoaderCircle class="spin" size={20}/><span>Loading diff...</span></div>
                  {:else if diffError}<div class="diff-empty" role="alert"><CircleAlert size={20}/><span>{diffError}</span><Button variant="secondary" onclick={()=>showDiff(selectedFile,current.id)}>Retry</Button></div>
                  {:else if diffCode}
                  <DiffView code={diffCode} label="Git diff"/>
                  {:else if selectedFile&&changedFiles.some(file=>file.path===selectedFile&&file.x==='?')}
                    <div class="diff-empty"><Code2 size={25}/><span>Untracked file</span><Button variant="secondary" disabled={working} onclick={()=>selectedFile&&runGit('stage_file',[selectedFile])}>Stage to preview diff</Button></div>
                  {:else}<div class="diff-empty"><Code2 size={25}/><span>{changedFiles.length?'No textual diff. The file may be binary or unchanged.':'No changes'}</span></div>{/if}
                </Card>
              </div>
              <div data-tour="commit" class="commit-panel workbench-commit">
                <div class="commit-heading">Commit changes</div>
                <p>{repoContext?.author ? 'Committing as '+repoContext.author : 'Set a Git author in Repository settings before committing.'}</p>
                {#if recoveredMessage}
                  <Collapsible>
                    {#snippet heading()}Recovered commit message{/snippet}
                    <pre>{recoveredMessage}</pre><Button variant="secondary" disabled={recoveredMessage.length>500} onclick={useRecoveredMessage}>Use recovered message</Button>
                  </Collapsible>
                {/if}
                <Textarea disabled={working||draftLoading} aria-label="Commit message" placeholder="Commit message" value={message} oninput={event=>updateDraft(event.currentTarget.value)} rows={2} maxlength={500}></Textarea>
                {#if messageTooLong}<p class="connection-error" role="alert">Shorten the commit message to 500 characters before committing.</p>{/if}
                {#if changedFiles.length}
                  <p aria-live="polite">{markedFiles.size===0 ? 'No files checked. All changes will be committed, including files hidden by filters.' : selectionMode==='exclude' ? `${markedCount} excluded · ${commitFiles.length} to commit` : markedCount===0 ? 'The selected files have changed. Select files again or clear your selection.' : `${commitFiles.length} selected to commit`}</p>
                {/if}
                <div class="toolbar inner">
                  {#if markedFiles.size}<Button variant="secondary" onclick={()=>markedFiles=new Set()} disabled={working}>Clear selection</Button>{/if}
                  <Button variant="primary" onclick={commit} disabled={working||draftLoading||messageTooLong||!message.trim()||!!current.status_error||!commitFiles.length||!!gitState?.operation||!!gitState?.conflicts.length}>Commit {commitFiles.length&&commitFiles.length===changedFiles.length?'all ':''}{commitFiles.length} {commitFiles.length===1?'file':'files'}</Button>
                </div>
              </div>
              </Tabs.Content>
            {:else if activeTab==='history'}
              <Tabs.Content value="history">
              {#if gitState?.head&&gitState.branch!=='(detached)'&&!gitState.dirty&&!gitState.operation&&!gitState.behind&&(!gitState.upstream||gitState.ahead>0)}
                <div class="toolbar"><Button variant="secondary" disabled={working} onclick={()=>{if(current&&gitState?.head)undoConfirm={repo:current,branch:gitState.branch,head:gitState.head};}}>Undo last local commit…</Button></div>
              {/if}
              {#key current.id}<CommitHistory repoId={current.id} head={gitState?.head??null} blocked={working||!!gitState?.dirty||!!gitState?.operation} onrevert={revertCommit}/>{/key}
              </Tabs.Content>
            {:else if activeTab==='pulls'}
              <Tabs.Content value="pulls">
              {#key current.path}<PullRequests target={targetOf(current)} git={gitState} {accounts} preferences={activeSettings} persistentDrafts={workflowApp.save_drafts} onconnect={openAccount} onchanged={()=>{void refresh();void loadMetadata();}}/>{/key}
              </Tabs.Content>
            {/if}
            </Tabs.Root>
            {#if actionResult}<Card class="panel"><div class="panel-title">Git output</div><pre class="output">{actionResult}</pre></Card>{/if}
            {#if repoActivity.length}<Card class="workbench-activity panel"><div class="panel-title">Recent activity</div>{#each repoActivity as item}<div class="activity-entry"><div class="activity-icon"><Activity size={15}/></div><div class="activity-text"><strong>{item.title}</strong><span>{short(item.detail??'',70)}</span></div><time>{date(item.occurred_at)}</time></div>{/each}</Card>{/if}
            <Button class="remove-repo" disabled={working} onclick={()=>removeRepo(current)}><X size={13}/> Remove from workspace</Button>
          </section>
        </div>
      {:else if view==='settings'}

        <div class="page-heading"><div><h1>Settings</h1><p>Manage GitHub connections and local configuration.</p></div></div>

        <Card class="panel"><div class="panel-title">GitHub accounts <span>{accounts.length} connected</span></div>{#each accounts as acc}<div class="account-row"><div class="account-avatar"><PlatformIcon remote="https://github.com" size={19}/></div><div class="account-info"><strong>@{acc.login}</strong><small>{acc.api_host}</small></div><Button class="micro" onclick={()=>act(()=>api.sync(acc.id),'Synchronized '+acc.login)}>Sync workflows</Button><Button class="micro dangerous" onclick={()=>disconnectAccount(acc)}>Disconnect</Button></div>{:else}<div class="empty-small">No GitHub accounts connected. Local Git works without one.</div>{/each}<div class="panel-footer"><Button data-tour="connect" variant="primary" onclick={openAccount}><Plus size={15}/> Connect account</Button></div></Card>

        <Card class="panel info-panel"><ShieldCheck size={21}/><div><strong>Credentials stay on your device</strong><p>GitHub access tokens are stored in your operating system's credential manager. Repository metadata, pins, and activity are stored locally in SQLite. Account connections support GitHub.com; local Git works with other hosts.</p></div></Card>

        <Card class="panel workspace-settings"><h2>Repository folder</h2><WorkspaceFolder settings={workspace} onsaved={settings=>{workspace=settings;++discoverySequence;found=null;discoveryError='';void scanDefault();}}/></Card>
        <Card class="panel workspace-settings startup-settings"><h2>Startup</h2><StartupSettings/></Card>
        <Card class="panel workspace-settings"><h2>Workflow defaults</h2>{#if workflowReady}<WorkflowSettings app={workflowApp} onsaved={value=>{workflowApp=value as AppPreferences;}}/>{:else}<Button variant="secondary" onclick={loadWorkflowApp}>Load workflow preferences</Button>{/if}</Card>
        <Card class="panel walkthrough-settings"><div><strong>A quick tour</strong><p>Revisit the basics whenever you need them.</p></div><Button variant="secondary" onclick={()=>showWalkthrough=true}>Open walkthrough</Button></Card>
        <Card class="panel walkthrough-settings"><div><strong>Live logs</strong><p>View runtime activity, Git diagnostics, and error details.</p></div><Button variant="secondary" onclick={()=>showLiveLogs=true}>Open live logs</Button></Card>

      {:else if view==='activity'}

        <div class="page-heading"><div><h1>Activity history</h1><p>Local actions and observed GitHub workflow runs.</p></div><Button variant="secondary" onclick={sync} disabled={working||accounts.length===0}><RefreshCw size={15}/> Sync GitHub</Button></div>

        <Card class="panel"><div class="panel-title">Recent events <span>{activities.length} events</span></div>{#each activities as item}<div class="activity-entry"><div class="activity-icon"><Activity size={15}/></div><div class="activity-text"><strong>{item.title}</strong><span>{item.repo_name??'GitHub'}{item.detail?' · '+short(item.detail,110):''}</span></div><time>{date(item.occurred_at)}</time></div>{:else}<div class="empty-small">No activity yet.</div>{/each}</Card>

      {:else}

        <div class="page-heading"><div><h1 class="overview-title">{view==='pinned'?'Pinned repositories':view==='attention'?'Needs attention':view.startsWith('collection:')?view.slice(11):'Overview'}</h1></div><div class="heading-actions"><Button variant="secondary" onclick={()=>openImport()}>Import from…</Button><Button data-tour="add" variant="primary" disabled={working} onclick={openAdd}><Plus size={16}/> Add repository</Button></div></div>

        {#if discoveryError}<div class="discovery-banner" role="alert"><span>{discoveryError}</span><Button variant="secondary" onclick={()=>view='settings'}>Choose folder</Button></div>{:else if found?.items.length}<div class="discovery-banner" role="status"><span>{found.items.length} new repositories found</span><Button variant="secondary" onclick={()=>openImport(found)}>Review & import</Button></div>{/if}
        <div class="shelf-tools">
          <div class="collection-chips" aria-label="Filter by collection"><Button class={[{"chip-active": view==='all'}]} onclick={()=>view='all'}>All</Button>{#each collections as c}<Button class={[{"chip-active": view==='collection:'+c}]} onclick={()=>view='collection:'+c}><Folder size={13}/>{c}</Button>{/each}</div>
          <div class="search-input shelf-search"><Search size={17}/><Input data-tour="search" bind:ref={searchInput} aria-label="Search repositories" placeholder="Search repositories" bind:value={query}/><kbd class="key-hint">Ctrl K</kbd></div>

        </div>
        <div class="shelf-count">{visible.length} {visible.length===1?'repository':'repositories'}</div>
        <div class="repository-shelf" aria-label="Repositories">
          {#each visible.slice((page-1)*pageSize,page*pageSize) as r (r.id)}
            <Card as="article" class="repository-card">
              <div class="card-heading"><Button class="card-open" onclick={()=>selectRepo(r.id)}><span class="card-icon"><PlatformIcon remote={r.remote_url} size={25}/></span><h2>{r.name}</h2></Button><Button class={["card-pin", {"is-pinned": r.pinned}]} aria-label={r.pinned?'Unpin '+r.name:'Pin '+r.name} title={r.pinned?'Unpin':'Pin'} disabled={pendingPins.includes(r.id)} onclick={()=>togglePin(r)}><Pin size={17}/></Button></div>
              <p class="card-path" title={r.path}><span class="platform-name">{remoteInfo(r.remote_url).label}</span> · {remoteInfo(r.remote_url).project||short(r.path,64)}</p>
              <div class="card-state"><span class="card-branch" title={r.branch}><GitBranch size={14}/>{r.branch||'—'}</span>{#if r.status_pending}<span class="pill">Checking…</span>{:else if r.missing}<span class="pill red">Folder missing</span>{:else if r.status_error}<span class="pill red" title={r.status_error}>Status unavailable</span>{:else if r.conflicts}<span class="pill red">Conflicts to resolve</span>{:else if r.dirty>0}<span class="pill yellow">{r.dirty} changed</span>{:else if r.ci_conclusion==='failure'}<span class="pill red">CI failed</span>{:else if r.ahead>0&&r.behind===0}<span class="pill yellow">Ready to push</span>{:else}<span class="pill green"><Check size={12}/> Clean</span>{/if}</div>
              <div class="card-counts"><span><ArrowUp size={13}/>{r.ahead} ahead</span><span><ArrowDown size={13}/>{r.behind} behind</span>{#if r.collection}<span class="card-collection" title={r.collection}>{r.collection}</span>{/if}</div>
              {#if metadata.get(r.id)?.fetched_at}<p class="card-fetch">Last fetched {date(metadata.get(r.id)!.fetched_at!)}</p>{/if}
              {#if r.status_error&&r.branch}<p class="card-fetch">Last known branch and counts</p>{/if}
              {#if launchFailures[r.id]}<p class="card-fetch">{launchFailures[r.id]}</p><Button variant="secondary" disabled={retryingLaunchFetch} onclick={()=>retryLaunchFetch(r.id)}>Retry fetch</Button>{/if}
              {#if r.missing}<Button variant="secondary" onclick={()=>locating=r}>Locate folder</Button>{/if}
              <footer class="card-footer">{#if activities.find(item=>item.repo_id===r.id)}<span><Clock3 size={12}/>{date(activities.find(item=>item.repo_id===r.id)!.occurred_at)}</span>{:else}<span class="card-tags">{r.tags.slice(0,3).join(' / ')}</span>{/if}{#if r.ci_conclusion||r.ci_status}<span class="card-ci"><Activity size={12}/>{r.ci_conclusion??r.ci_status}</span>{/if}</footer>
            </Card>
          {:else}<div class="empty-large shelf-empty"><div class="empty-illustration"><FolderGit2 size={32} strokeWidth={1.5}/></div><h3>{loading?'Opening workspace...':repos.length===0?'No repositories yet':'No repositories found'}</h3><p>{repos.length===0?'Add an existing local Git repository.':'Try a different search or collection.'}</p><Button data-tour="add" variant="primary" disabled={working} onclick={openAdd}><Plus size={16}/> Add repository</Button></div>{/each}
        </div>
        {#if visible.length>pageSize}<div class="pagination"><Button variant="secondary" disabled={page===1} onclick={()=>page--}>Previous</Button><span>Page {page} of {Math.ceil(visible.length/pageSize)}</span><Button variant="secondary" disabled={page*pageSize>=visible.length} onclick={()=>page++}>Next</Button></div>{/if}

        <Card as="section" class="latest-activity panel" aria-label="Latest activity"><div class="latest-heading"><h2><Clock3 size={17}/> Latest activity</h2><Button class="text-action" onclick={()=>view='activity'}>View all activity <ArrowUp size={13}/></Button></div><div class="latest-items">{#each activities.slice(0,3) as item}<div class="latest-item"><span class="activity-icon">{#if item.kind==='ci'}<Activity size={16}/>{:else}<GitCommitHorizontal size={16}/>{/if}</span><div><strong>{item.repo_name??'GitHub'} &middot; {item.title}</strong><small>{date(item.occurred_at)}</small></div></div>{:else}<div class="empty-small">No activity yet</div>{/each}</div></Card>
      {/if}

    </main>

  </div>

</div>

{#if showAdd}<AddRepository {accounts} defaultFolder={workspace.default_folder} onclose={closeAdd} onconnect={()=>{closeAdd();openAccount();}} oncreated={repositoryAdded}/>{/if}

{#if showOrganize}<Modal label="Organize repository" canClose={!organizing} onclose={()=>{showOrganize=false;}}><div class="modal-top"><div><h2>Organize {current?.name??'repository'}</h2></div><Button class="top-icon" aria-label="Close dialog" onclick={()=>showOrganize=false}><X size={18}/></Button></div><TokenInput label="Collection" bind:items={collection} maxItems={1} maxLength={100}/><TokenInput label="Tags" bind:items={tags}/><div class="modal-actions"><Button variant="secondary" onclick={()=>showOrganize=false}>Cancel</Button><Button variant="primary" onclick={organize} disabled={organizing}>Save changes</Button></div></Modal>{/if}

{#if showAccount}<Modal label="Connect GitHub" onclose={closeAccount}><div class="modal-top"><div><h2>Connect GitHub</h2></div><Button class="top-icon" aria-label="Close dialog" onclick={closeAccount}><X size={18}/></Button></div>
  {#if loginError}<p class="connection-error" role="alert">{loginError}</p>{/if}
  {#if accountMode==='browser'}
    {#if loginCode}<p>Enter this code on GitHub:</p><div class="device-code" aria-label="GitHub sign-in code">{loginCode.user_code}</div><p class="login-status" role="status"><LoaderCircle size={15} class="spin"/>{loginMessage||'Waiting for GitHub…'}</p><Button class="secondary browser-reopen" onclick={reopenLogin}>Open GitHub</Button>
    {:else}<p>Continue in your browser to connect your account and manage public repositories and pull requests.</p>{#if browserAvailable===false}<p class="connection-error">Browser sign-in needs app setup. You can use a token below.</p>{/if}<small>GitHub access includes public repository writes and organization membership.</small><Label class="scope-choice"><Checkbox bind:checked={privateRepos} disabled={loginBusy}/><span>Include private repositories</span></Label>{#if privateRepos}<small>GitHub's repository permission also grants write access. Reponiq uses this connection for repositories, workflows and pull requests.</small>{/if}<div class="modal-actions"><Button variant="secondary" onclick={closeAccount}>Cancel</Button><Button variant="primary" onclick={browserConnect} disabled={loginBusy||working||!browserAvailable}>{#if loginBusy}<LoaderCircle size={15} class="spin"/>{:else}<Github size={16}/>{/if} Continue with GitHub</Button></div><Button class="text-action auth-alternative" disabled={loginBusy} onclick={()=>{accountMode='token';loginError='';}}>Use a personal access token</Button>{/if}
  {:else}<Label for="token">Personal access token</Label><Input id="token" type="password" placeholder="github_pat_…" autocomplete="off" bind:value={token}/><small>For private workflows, use a fine-grained token with Actions read access.</small><div class="modal-actions"><Button variant="secondary" onclick={()=>{accountMode='browser';token='';loginError='';}}>Back</Button><Button variant="primary" onclick={connect} disabled={working||!token.trim()}>Connect account</Button></div>{/if}
</Modal>{/if}

{#if showBranch}<Modal label="Create branch" onclose={()=>showBranch=false}><h2>Create branch</h2><Label for="branch-name">Branch name</Label><Input id="branch-name" bind:value={newBranch} placeholder="feature/name" onkeydown={event=>{if(event.key==='Enter'&&!event.isComposing)void createBranch().then(()=>{if(!newBranch)showBranch=false;});}}/><div class="modal-actions"><Button variant="secondary" onclick={()=>showBranch=false}>Cancel</Button><Button variant="primary" disabled={working||!newBranch.trim()} onclick={()=>createBranch().then(()=>{if(!newBranch)showBranch=false;})}>Create & switch</Button></div></Modal>{/if}
{#if conflictFile&&current}{#key current.id+':'+conflictFile}<ConflictEditor repoId={current.id} file={conflictFile} expected={gitState?{branch:gitState.branch,head:gitState.head}:null} onclose={()=>conflictFile=null} onsaved={()=>void refresh()}/>{/key}{/if}

{#if pendingGit}<Modal label={pendingGit.title} onclose={()=>pendingGit=null}><h2>{pendingGit.title}</h2><p class="confirm-description">{pendingGit.description}</p><p class="confirm-context">{repos.find(repo=>repo.id===pendingGit!.id)?.name} · {pendingGit.expected.branch}</p><div class="modal-actions"><Button variant="secondary" onclick={()=>pendingGit=null}>Cancel</Button><Button variant="primary" onclick={executePendingGit}>Confirm</Button></div></Modal>{/if}

{#if showWalkthrough}<Walkthrough hasRepo={repos.length>0} prepare={prepareTour} onclose={()=>showWalkthrough=false}/>{/if}
{#if showLiveLogs}<LiveLogs onclose={()=>showLiveLogs=false}/>{/if}

{#if showBranches && details}<Branches {details} {error} {recentBranches} defaultBranch={repoContext?.default_branch??null} busy={working} onclose={()=>showBranches=false} onswitch={(branch,strategy)=>runGit('switch_branch',[branch.reference,strategy])} onfetch={()=>runGit('fetch')} oncreate={()=>{showBranches=false;newBranch='';showBranch=true;}} onrestore={(oid)=>runGit('restore_changes',[oid])} onopenworktree={reference=>{if(current)void openLocation(current,'worktree',reference);}}/>{/if}
{#if showQuick}<QuickSwitch repos={orderRepos(repos)} {metadata} canOpen={!working} onselect={repo=>{showQuick=false;void selectRepo(repo.id);}} onclose={()=>showQuick=false} onmove={arrangePin} onlocation={(repo,action)=>{showQuick=false;void openLocation(repo,action);}}/>{/if}
{#if showRepoSettings&&current}
  <Modal label="Repository settings" onclose={()=>showRepoSettings=false}>
    <h2>{current.name} settings</h2>
    {#key current.path}
      <WorkflowSettings target={targetOf(current)} app={workflowApp} preferences={activeSettings??null} context={repoContext} onsaved={value=>{
        if(current){const next=new Map(metadata);next.set(current.id,{id:current.id,path:current.path,pr_login:null,opened_at:null,pin_position:null,fetched_at:null,main_worktree:null,cached_status:null,...activeSettings,...(value as RepoPreferences)});metadata=next;}
        void loadMetadata();
      }} onidentity={value=>repoContext=value}/>
    {/key}
  </Modal>
{/if}
{#if locating}
  <LocateRepo repo={locating} mainWorktree={metadata.get(locating.id)?.main_worktree} onclose={()=>locating=null} onlocated={async()=>{
    const previous=locating;if(!previous)return;locating=null;
    statusVersions.set(previous.id,(statusVersions.get(previous.id)??0)+1);
    await loadFlight;
    await load();await loadMetadata();
    if(selected===previous.id)await selectRepo(previous.id);
  }} onadded={async repo=>{locating=null;await repositoryAdded(repo);}}/>
{/if}
{#if defaultCommit}
  <Modal label="Commit on the default branch" onclose={()=>defaultCommit=null}>
    <h2>Commit on {defaultCommit.branch}?</h2>
    <p>Create a branch to keep this work separate, or commit {defaultCommit.files.length} {defaultCommit.files.length===1?'file':'files'} directly to {defaultCommit.branch}.</p>
    <Label>New branch name<Input bind:value={branchNameForCommit} placeholder="feature/my-change"/></Label>
    <div class="modal-actions">
      <Button class="text-action" onclick={()=>defaultCommit=null}>Cancel</Button>
      <Button variant="secondary" onclick={()=>acceptDefaultCommit(false)}>Commit here</Button>
      <Button variant="primary" disabled={!branchNameForCommit.trim()} onclick={()=>acceptDefaultCommit(true)}>Create branch with my edits</Button>
    </div>
  </Modal>
{/if}
{#if undoConfirm}
  <Modal label="Undo last commit" onclose={()=>undoConfirm=null}>
    <h2>Undo the last commit?</h2>
    <p>Your changes will stay staged on {undoConfirm.branch}. Reponiq checks whether the commit is already shared before undoing it.</p>
    <div class="modal-actions"><Button variant="secondary" onclick={()=>undoConfirm=null}>Cancel</Button><Button variant="primary" onclick={undoCommit}>Undo commit and keep changes</Button></div>
  </Modal>
{/if}
{#if showSetup}<Modal label="Set up repositories" canClose={!setupBusy} onclose={skipSetup}><h2>Choose your repository folder</h2><p>You can change this later in Settings.</p><WorkspaceFolder settings={workspace} initial bind:busy={setupBusy} onsaved={workspaceSaved}/></Modal>{/if}
{#if showStartup}<Modal label="Startup preferences" canClose={!startupBusy} onclose={finishSetup}><h2>Startup preferences</h2><p>Change these options any time in Settings.</p><StartupSettings initial bind:busy={startupBusy} ondone={finishSetup}/></Modal>{/if}
{#if showImport}<ImportRepositories initial={importInitial} defaultFolder={workspace.default_folder} onclose={()=>{showImport=false;void load();}} onimported={imported}/>{/if}
<ConfirmDialog/>
