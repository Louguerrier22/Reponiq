<script lang="ts">
  import Button from './ui/Button.svelte';
  import Collapsible from './ui/Collapsible.svelte';
  import { confirm } from './ui/confirmation.svelte';
  import Card from './ui/Card.svelte';
  import Checkbox from './ui/Checkbox.svelte';
  import Input from './ui/Input.svelte';
  import Label from './ui/Label.svelte';
  import NativeSelect from './ui/NativeSelect.svelte';
  import Textarea from './ui/Textarea.svelte';
  import {untrack,onDestroy} from 'svelte';
  import {GitPullRequest,RefreshCw,Plus,Search,FileCode,Check,CircleAlert} from 'lucide-svelte';
  import {api,type Account,type GitState,type Pull,type PullDetails,type PullFile,type PullCommit,type PullContext,type PullComparison,type Target,type RepoWorkflow} from './api';
  import {draftKey,readDraft,saveDraft,type DraftContext} from './draft-store';
  import DiffView from './DiffView.svelte';
  import Modal from './Modal.svelte';
  let {target,git,accounts,preferences,persistentDrafts,onconnect,onchanged}:{
    target:Target;git:GitState|null;accounts:Account[];preferences?:RepoWorkflow;
    persistentDrafts:boolean;onconnect:()=>void;onchanged:()=>void}=$props();
  const repoId=$derived(target.id);
  let accountId=$state(0),page=$state(1),hasMore=$state(false),rows=$state<Pull[]>([]),context=$state<PullContext|null>(null),selected=$state<Pull|null>(null),detail=$state<PullDetails|null>(null);
  let loading=$state(false),detailLoading=$state(false),busy=$state(false),error=$state(''),notice=$state(''),query=$state('');
  let files=$state<PullFile[]>([]),filePage=$state(1),moreFiles=$state(false),fileQuery=$state(''),selectedFile=$state<PullFile|null>(null),filesLoading=$state(false);
  let commits=$state<PullCommit[]>([]),commitsLimited=$state(false),commit=$state('');
  let reviewEvent=$state('COMMENT'),reviewBody=$state(''),reviewQuery=$state(''),reviewPage=$state(0),checkQuery=$state(''),checkPage=$state(0),method=$state('merge');
  let showCreate=$state(false),title=$state(''),body=$state(''),base=$state(''),draft=$state(false);
  let accountPicked=$state(false),draftLoading=$state(false),draftVersion=0;
  const account=$derived(accounts.find(account=>account.id===accountId));
  const composeScope=$derived.by(():DraftContext|null=>{
    if(!showCreate||!git?.branch||git.branch==='(detached)'||!account||!base.trim())return null;
    return {target,scope:{branch:git.branch,account:account.login,base:base.trim()}};
  });
  const composeKey=$derived(composeScope&&draftKey(composeScope));
  let comparison=$state<{repo:number;account:number;base:string;branch:string;head:string;result:PullComparison}|null>(null);
  let comparing=$state(false);
  let pendingMerge=$state<{number:number;head:string;method:string;title:string}|null>(null);
  let listSequence=0,detailSequence=0,fileSequence=0,alive=true;
  const filtered=$derived(rows.filter(p=>[p.title,p.user.login,String(p.number),p.head.ref,p.base.ref].some(s=>s.toLocaleLowerCase().includes(query.toLocaleLowerCase()))));
  const visibleFiles=$derived(files.filter(f=>f.filename.toLocaleLowerCase().includes(fileQuery.toLocaleLowerCase())));
  const reviews=$derived((detail?.reviews??[]).filter(r=>[r.user.login,r.state,r.body??''].some(s=>s.toLocaleLowerCase().includes(reviewQuery.toLocaleLowerCase()))).slice().reverse());
  const checks=$derived((detail?.checks.runs??[]).filter(c=>[c.name,c.status,c.conclusion??''].some(s=>s.toLocaleLowerCase().includes(checkQuery.toLocaleLowerCase()))));
  const blockedReview=$derived.by(()=>{const latest=new Map<string,string>();for(const r of detail?.reviews??[])if(['APPROVED','CHANGES_REQUESTED','DISMISSED'].includes(r.state))latest.set(r.user.login,r.state);return [...latest.values()].includes('CHANGES_REQUESTED');});
  const canMerge=$derived(!busy&&!detailLoading&&!!detail&&detail.pull.state==='open'&&!detail.pull.draft&&detail.pull.mergeable===true&&['clean','has_hooks'].includes(detail.pull.mergeable_state??'')&&detail.checks.ready&&detail.checks.complete&&detail.reviews_complete&&!blockedReview&&detail.merge_methods.includes(method));
  const canCompare=$derived(!busy&&!comparing&&!!git?.head&&!git.dirty&&!git.operation&&!git.conflicts.length&&!!git.upstream&&!git.upstream_gone&&git.ahead===0&&!!base.trim()&&git.branch!==base);
  const currentComparison=$derived(comparison?.repo===repoId&&comparison?.account===accountId&&comparison?.base===base&&comparison?.head===git?.head&&comparison?.branch===git?.branch ? comparison.result : null);
  const canCreate=$derived(canCompare&&!draftLoading&&!!currentComparison?.ahead_by);
  $effect(()=>{
    const list=accounts;
    const remembered=preferences?.pr_login;
    if(accountPicked){if(accountId&&!list.some(a=>a.id===accountId))accountId=0;return;}
    accountId=remembered?(list.find(a=>a.login.toLocaleLowerCase()===remembered.toLocaleLowerCase())?.id??0):(list.length===1?list[0].id:0);
  });
  $effect(()=>{repoId;accountId;untrack(()=>{if(accountId)void loadList(1);else{loading=false;rows=[];context=null;selected=null;detail=null;++listSequence;++detailSequence;++fileSequence;}});});
  $effect(()=>{
    const key=composeKey;
    if(key)void untrack(()=>restoreDraft(key));
    else {++draftVersion;title='';body='';draftLoading=false;}
  });
  $effect(()=>{reviewQuery;reviewPage=0;});$effect(()=>{checkQuery;checkPage=0;});
  onDestroy(()=>{alive=false;++listSequence;++detailSequence;++fileSequence;});
  async function loadList(next:number){
    if(!accountId)return;const sequence=++listSequence;const id=repoId,account=accountId;loading=true;error='';selected=null;detail=null;files=[];commits=[];++detailSequence;++fileSequence;
    const results=await Promise.allSettled([api.pulls(id,account,next),api.pullContext(id,account)]);
    if(!alive||sequence!==listSequence)return;
    if(results[0].status==='fulfilled'){rows=results[0].value.items;hasMore=results[0].value.has_more;page=next;if(rows[0])void choose(rows[0]);}else{rows=[];error=String(results[0].reason);}
    if(results[1].status==='fulfilled'){context=results[1].value;}else{context=null;error=String(results[1].reason);}
    loading=false;
  }
  async function choose(pull:Pull){
    const sequence=++detailSequence;const id=repoId,account=accountId;const same=selected?.number===pull.number;selected=pull;detail=null;files=[];selectedFile=null;commits=[];commit='';detailLoading=true;error='';++fileSequence;if(!same){reviewBody='';reviewEvent='COMMENT';}reviewPage=0;checkPage=0;
    const result=await Promise.allSettled([api.pullDetails(id,account,pull.number),api.pullCommits(id,account,pull.number)]);
    if(!alive||sequence!==detailSequence)return;
    if(result[0].status==='fulfilled'){detail=result[0].value;selected=detail.pull;method=detail.merge_methods[0]??'merge';void loadFiles(1);}else error=String(result[0].reason);
    if(result[1].status==='fulfilled'){commits=result[1].value.items;commitsLimited=result[1].value.has_more;}else error=String(result[1].reason);
    detailLoading=false;
  }
  async function loadFiles(next:number){if(!selected)return;const sequence=++fileSequence;const id=repoId,account=accountId,number=selected.number,sha=commit||null;filesLoading=true;selectedFile=null;fileQuery='';try{const result=await api.pullFiles(id,account,number,next,sha);if(alive&&sequence===fileSequence){files=result.items;moreFiles=result.has_more;filePage=next;selectedFile=files[0]??null;}}catch(e){if(alive&&sequence===fileSequence){files=[];error=String(e);}}finally{if(alive&&sequence===fileSequence)filesLoading=false;}}
  function openCreate() {
    error=''; title=''; body=''; draft=false; comparison=null;
    const preferred=preferences?.preferred_base;
    base=preferred ? context?.branches.includes(preferred)?preferred:'' : context?.default_branch??'';
    if(preferred&&!base)error='Your preferred base is unavailable in the branch list. Choose or enter a base.';
    showCreate=true;
  }
  async function restoreDraft(key:string){
    const scope=composeScope;if(!scope)return;
    const revision=++draftVersion;
    title='';body='';draftLoading=true;
    try{
      const value=await readDraft(scope,persistentDrafts);
      if(alive&&revision===draftVersion&&composeKey===key){title=value?.title??'';body=value?.body??'';}
    }catch(reason){if(alive&&revision===draftVersion)error=String(reason);}
    finally{if(alive&&revision===draftVersion)draftLoading=false;}
  }
  function persistDraft(){
    if(composeScope)void saveDraft(composeScope,{title,body},persistentDrafts).catch(reason=>{error=String(reason);});
  }
  async function discardDraft(){
    const scope=composeScope;
    if(!scope || !await confirm({title:'Discard this pull request draft?',action:'Discard draft',destructive:true}) || scope!==composeScope)return;
    title='';body='';persistDraft();
  }
  async function openFailedCheck(number:number,webUrl:string|null){
    try{await api.openCheck(repoId,number,webUrl);}
    catch(reason){error=String(reason);}
  }
  async function compare() {
    if (!canCompare || !git?.head) return;
    const snapshot={repo:repoId,account:accountId,base,branch:git.branch,head:git.head};
    comparing=true; error=''; comparison=null;
    try {
      const result=await api.comparePull(snapshot.repo,snapshot.account,snapshot.base,snapshot.branch,snapshot.head);
      if (alive && showCreate) comparison={...snapshot,result};
    } catch (reason) {if (alive) error=String(reason);}
    finally {comparing=false;}
  }
  async function create(){
    if(!git?.head||!canCreate||!title.trim()||!currentComparison)return;
    const input={title,body,base,head:git.branch,expected_head:git.head,expected_base:currentComparison.base_commit.sha,draft};
    const id=repoId,account=accountId;
    const scope=composeScope;
    busy=true; error='';
    try {
      const {pull,matches_comparison}=await api.createPull(id,account,input);
      notice=matches_comparison?'Created pull request #'+pull.number:'';
      showCreate=false; title=''; body='';
      let draftError='';
      if(scope&&matches_comparison)await saveDraft(scope,null,persistentDrafts).catch(reason=>{draftError=String(reason);});
      await loadList(1); await choose(pull);
      if(!matches_comparison)error=`GitHub created pull request #${pull.number} after a branch changed. Review the new pull request before continuing.`;
      else if(draftError)error=draftError;
    } catch(reason) {
      comparison=null;
      error=String(reason);
    } finally {busy=false;}
  }
  async function review(){if(!detail||busy||detailLoading)return;const number=detail.pull.number,head=detail.pull.head.sha,id=repoId,account=accountId,event=reviewEvent,text=reviewBody;busy=true;error='';try{await api.reviewPull(id,account,number,head,event,text);notice='Review submitted for #'+number;reviewBody='';if(selected?.number===number)await choose(selected);}catch(e){error=String(e);}finally{busy=false;}}
  function confirmMerge(){if(!detail||!canMerge)return;pendingMerge={number:detail.pull.number,head:detail.pull.head.sha,method,title:detail.pull.title};}
  async function merge(){if(!pendingMerge||busy)return;const snapshot=pendingMerge,id=repoId,account=accountId;busy=true;error='';try{notice=await api.mergePull(id,account,snapshot.number,snapshot.head,snapshot.method)+' Fetch to update your local refs.';pendingMerge=null;onchanged();await loadList(1);}catch(e){const message=String(e);pendingMerge=null;if(selected)await choose(selected);error=message;}finally{busy=false;}}
</script>

<section class="pull-requests" aria-label="Pull requests">
  <div class="pr-toolbar"><Label>GitHub account<NativeSelect aria-label="GitHub account for pull requests" bind:value={accountId} onchange={()=>{accountPicked=true;}} disabled={busy||loading}><option value={0}>Choose an account</option>{#each accounts as account}<option value={account.id}>@{account.login}</option>{/each}</NativeSelect></Label><div class="toolbar"><Button variant="secondary" disabled={busy||loading||!accountId} onclick={()=>loadList(page)}><RefreshCw size={15}/> Refresh</Button><Button variant="primary" disabled={!accountId||!context||busy} onclick={openCreate}><Plus size={15}/> New pull request</Button></div></div>
  {#if notice}<p class="inline-success" role="status">{notice}</p>{/if}{#if error}<p class="connection-error" role="alert">{error}</p>{/if}
  {#if accounts.length&&!accountId}<p>Select a connected GitHub account to view pull requests.</p>{/if}
  {#if !accounts.length}<div class="empty-block"><GitPullRequest size={28}/><p>Connect GitHub to review and manage pull requests.</p><Button variant="primary" onclick={onconnect}>Connect GitHub</Button></div>
  {:else}<div class="pr-workbench"><Card as="section" class="panel pr-list"><div class="panel-title">Open pull requests <span>{context?.full_name??''}</span></div><div class="search-input history-search"><Search size={15}/><Input aria-label="Find pull requests" placeholder="Find in this page" bind:value={query}/></div><div class="commit-list">{#each filtered as pull (pull.number)}<Button class={["commit-choice", {"is-selected": selected?.number===pull.number}]} disabled={busy} onclick={()=>{if(selected?.number!==pull.number)void choose(pull);}}><GitPullRequest size={18}/><span><strong>{pull.title}</strong><small>#{pull.number} · @{pull.user.login}{pull.draft?' · Draft':''}</small><code>{pull.head.ref} → {pull.base.ref}</code></span></Button>{:else}<p class="empty-small">{loading?'Loading pull requests…':query?'No matching pull requests':'No open pull requests'}</p>{/each}</div><div class="history-paging"><Button variant="secondary" disabled={busy||loading||page===1} onclick={()=>loadList(page-1)}>Previous</Button><span>{page}</span><Button variant="secondary" disabled={busy||loading||!hasMore} onclick={()=>loadList(page+1)}>Next</Button></div></Card>
  <div class="pr-inspector">{#if detail&&selected}<Card as="section" class="panel pr-summary"><div class="pr-title"><div><h2>#{detail.pull.number} {detail.pull.title}</h2><p>@{detail.pull.user.login} · {detail.pull.head.label} → {detail.pull.base.ref}</p></div><span class="pill" class:green={detail.pull.mergeable===true&&!detail.pull.draft} class:yellow={detail.pull.draft||detail.pull.mergeable!==true}>{detail.pull.draft?'Draft':detail.pull.mergeable===false?'Conflicts':detail.pull.mergeable===null?'Checking':'Open'}</span></div>{#if detail.pull.body}<pre class="pr-description">{detail.pull.body.slice(0,20000)}</pre>{/if}<code class="pr-head">Reviewed head: {detail.pull.head.sha.slice(0,12)}</code>
  <Collapsible class="pr-status">{#snippet heading()}{#if detail?.checks.ready}<Check size={15}/> Checks passed or not configured{:else}<CircleAlert size={15}/> Checks pending, failing or unavailable{/if}{/snippet}{#if detail.checks.runs.length>10}<Input aria-label="Filter checks" placeholder="Filter checks" bind:value={checkQuery}/>{/if}<div class="check-list">{#each checks.slice(checkPage*10,checkPage*10+10) as check}<div><span>{check.name}</span><b class:change-added={check.conclusion==='success'} class:change-deleted={['failure','timed_out','cancelled'].includes(check.conclusion??'')}>{check.conclusion??check.status}</b>{#if check.conclusion&&!['success','neutral','skipped'].includes(check.conclusion)}<Button class="text-action" onclick={()=>detail&&openFailedCheck(detail.pull.number,check.web_url??null)}>Open check</Button>{/if}</div>{/each}<p>Commit status: {detail.checks.legacy_state}</p>{#if !detail.checks.ready}<Button class="text-action" onclick={()=>detail&&openFailedCheck(detail.pull.number,null)}>Open checks on GitHub</Button>{/if}{#if !detail.checks.complete}<p>Check results are incomplete. Merge is blocked.</p>{/if}</div>{#if checks.length>10}<div class="history-paging"><Button variant="secondary" disabled={checkPage===0} onclick={()=>checkPage--}>Previous</Button><span>{checkPage+1}</span><Button variant="secondary" disabled={(checkPage+1)*10>=checks.length} onclick={()=>checkPage++}>Next</Button></div>{/if}</Collapsible>
  </Card>
  <Card as="section" class="panel pr-diff"><div class="pr-commit-picker"><Label>Changes<NativeSelect aria-label="Pull request commit" bind:value={commit} onchange={()=>loadFiles(1)}><option value="">All changes</option>{#each commits as item}<option value={item.sha}>{item.sha.slice(0,8)} · {item.commit.message.split('\n')[0].slice(0,100)}</option>{/each}</NativeSelect></Label>{#if commitsLimited}<small>First 100 commits. Use GitHub for older or additional commits.</small>{/if}</div><div class="history-files-and-diff"><section class="history-files"><div class="panel-title">Files · page {filePage}</div>{#if files.length>8}<Input aria-label="Filter pull request files" placeholder="Filter files" bind:value={fileQuery}/>{/if}<div class="file-list">{#each visibleFiles as file (file.filename)}<Button class={["changed-file", {"file-selected": selectedFile?.filename===file.filename}]} onclick={()=>selectedFile=file}><FileCode size={15}/><span>{file.filename}</span><small class="change-added">+{file.additions}</small><small class="change-deleted">−{file.deletions}</small></Button>{:else}<p class="empty-small">{filesLoading?'Loading files…':'No files on this page'}</p>{/each}</div><div class="history-paging"><Button variant="secondary" disabled={filesLoading||filePage===1} onclick={()=>loadFiles(filePage-1)}>Previous</Button><Button variant="secondary" disabled={filesLoading||!moreFiles} onclick={()=>loadFiles(filePage+1)}>Next</Button></div></section><section class="diff-panel"><div class="panel-title"><span class="diff-filename">{selectedFile?.filename??'Select a file'}</span></div>{#if filesLoading}<div class="diff-empty">Loading diff…</div>{:else if selectedFile?.patch}<DiffView code={selectedFile.patch} label="Pull request diff"/>{:else}<div class="diff-empty">No textual preview. The file may be binary or GitHub may have omitted its patch.</div>{/if}</section></div></Card>
  <Card as="section" class="panel pr-review"><h3>Submit a review</h3><div class="inline-form"><NativeSelect aria-label="Review action" bind:value={reviewEvent} disabled={busy}><option value="COMMENT">Comment</option><option value="APPROVE">Approve</option><option value="REQUEST_CHANGES">Request changes</option></NativeSelect></div><Textarea aria-label="Review comment" placeholder="Review comment" maxlength={20000} rows={3} bind:value={reviewBody} disabled={busy}></Textarea><Button variant="primary" disabled={busy||detailLoading||detail.pull.state!=='open'||(reviewEvent!=='APPROVE'&&!reviewBody.trim())} onclick={review}>{reviewEvent==='APPROVE'?'Approve pull request':reviewEvent==='REQUEST_CHANGES'?'Request changes':'Submit comment'}</Button></Card>
  <Card as="section" class="panel pr-review-history"><h3>Reviews</h3>{#if detail.reviews.length>10}<Input aria-label="Filter reviews" placeholder="Filter reviews" bind:value={reviewQuery}/>{/if}{#each reviews.slice(reviewPage*10,reviewPage*10+10) as review}<article><strong>@{review.user.login}</strong><span class:change-added={review.state==='APPROVED'} class:change-deleted={review.state==='CHANGES_REQUESTED'}>{review.state.replaceAll('_',' ')}</span>{#if review.body}<pre>{review.body.slice(0,20000)}</pre>{/if}</article>{:else}<p>No reviews</p>{/each}{#if reviews.length>10}<div class="history-paging"><Button variant="secondary" disabled={reviewPage===0} onclick={()=>reviewPage--}>Previous</Button><span>{reviewPage+1}</span><Button variant="secondary" disabled={(reviewPage+1)*10>=reviews.length} onclick={()=>reviewPage++}>Next</Button></div>{/if}{#if !detail.reviews_complete}<p>Review history is incomplete; merge is blocked.</p>{/if}</Card>
  <Card as="section" class="panel pr-merge"><div><h3>Merge pull request</h3><p>{canMerge?'GitHub policy and current checks allow a merge.':'Merge is blocked until conflicts, checks, reviews and repository rules are satisfied.'}</p></div><NativeSelect aria-label="Merge method" bind:value={method} disabled={busy}>{#each detail.merge_methods as option}<option value={option}>{option==='squash'?'Squash and merge':option==='rebase'?'Rebase and merge':'Merge commit'}</option>{/each}</NativeSelect><Button variant="primary" disabled={!canMerge} onclick={confirmMerge}>Merge…</Button></Card>
  {:else if detailLoading}<div class="empty-block" role="status">Loading pull request…</div>{/if}</div></div>{/if}
</section>
{#if showCreate}
  <Modal canClose={!busy && !comparing} label="Create pull request" onclose={()=>showCreate=false}>
    <h2>Create pull request</h2>
    <p>{context?.full_name} · From <strong>{git?.branch??'No branch selected'}</strong></p>
    {#if account}<p>Creating as @{account.login}</p>{/if}
    {#if error}<p class="connection-error" role="alert">{error}</p>{/if}
    <Label for="pr-base">Base branch</Label>
    <Input id="pr-base" list="pr-base-options" bind:value={base} disabled={busy||comparing} oninput={()=>{comparison=null;error='';}}/>
    <datalist id="pr-base-options">{#each context?.branches??[] as branch}<option value={branch}></option>{/each}</datalist>
    {#if !git?.head || git.dirty || git.operation || git.conflicts.length || !git.upstream || git.upstream_gone || git.ahead}
      <p>Commit and publish your latest changes before comparing branches.</p>
    {:else if git.branch===base}
      <p>Choose the branch that should receive these changes.</p>
    {/if}
    {#if comparing}<p role="status">Comparing published changes…</p>{/if}
    {#if currentComparison}
      <p role="status">{currentComparison.ahead_by} {currentComparison.ahead_by === 1 ? 'new commit' : 'new commits'} · {currentComparison.behind_by} behind {base}</p>
      {#if currentComparison.files.length}
        <Collapsible open>{#snippet heading()}Changed files {currentComparison.files.length===300?'(first 300 from GitHub)':''}{/snippet}
          <ul class="branch-results">{#each currentComparison.files as file}<li>{file.filename} · {file.status}</li>{/each}</ul>
        </Collapsible>
      {/if}
      {#if currentComparison.ahead_by}
        <Label for="pr-title">Title</Label><Input id="pr-title" bind:value={title} oninput={persistDraft} maxlength={256} disabled={busy||draftLoading}/>
        <Label for="pr-body">Description</Label><Textarea id="pr-body" bind:value={body} oninput={persistDraft} rows={3} maxlength={20000} disabled={busy||draftLoading}></Textarea>
        <Label class="scope-choice"><Checkbox bind:checked={draft} disabled={busy}/>Draft pull request</Label>
      {:else}<p>This branch has no new commits for {base}. Choose another base.</p>{/if}
    {/if}
    <div class="modal-actions">
      <Button variant="secondary" disabled={busy||comparing} onclick={()=>showCreate=false}>Cancel</Button>
      {#if title||body}<Button class="text-action" disabled={busy||comparing||draftLoading} onclick={discardDraft}>Discard draft</Button>{/if}
      {#if currentComparison?.ahead_by}
        <Button variant="primary" disabled={!canCreate||!title.trim()} onclick={create}>Create pull request</Button>
      {:else}
        <Button variant="primary" disabled={!canCompare} onclick={compare}>Compare changes</Button>
      {/if}
    </div>
  </Modal>
{/if}
{#if pendingMerge}<Modal canClose={!busy} label="Confirm pull request merge" onclose={()=>{if(!busy)pendingMerge=null;}}><h2>Merge #{pendingMerge.number}?</h2><p>{pendingMerge.title}</p><p>Method: {pendingMerge.method} · Head: {pendingMerge.head.slice(0,12)}</p><p>GitHub status and the reviewed commit will be checked again before merging.</p><div class="modal-actions"><Button variant="secondary" disabled={busy} onclick={()=>pendingMerge=null}>Cancel</Button><Button variant="primary" disabled={busy} onclick={merge}>Merge pull request</Button></div></Modal>{/if}
