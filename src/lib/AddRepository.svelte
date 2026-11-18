<script lang="ts">
  import Button from './ui/Button.svelte';
  import TabsList from './ui/TabsList.svelte';
  import TabsTrigger from './ui/TabsTrigger.svelte';
  import { Tabs } from 'bits-ui';
  import Input from './ui/Input.svelte';
  import Label from './ui/Label.svelte';
  import NativeSelect from './ui/NativeSelect.svelte';
  import { onDestroy,onMount } from 'svelte';
  import { FolderOpen, Plus, Download, ArrowLeft, Check, ChevronDown, LoaderCircle, Lock, Globe, X } from 'lucide-svelte';
  import Modal from './Modal.svelte';
  import PlatformIcon from './PlatformIcon.svelte';
  import { cloneFolderName } from './clone';
  import { api,readablePath, type Account, type Repo, type OnlineRepo, type RepositoryOwner } from './api';
  let {accounts,defaultFolder=null,onclose,onconnect,oncreated}:{accounts:Account[];defaultFolder?:string|null;onclose:()=>void;onconnect:()=>void;oncreated:(repo:Repo,warning?:string|null)=>Promise<void>}=$props();
  let mode=$state<'choose'|'folder'|'clone'|'create'>('choose'),busy=$state(false),error=$state('');
  let folder=$state(''),parent=$state(''),name=$state(''),accountId=$state(0),owner=$state(''),isPrivate=$state(true);
  let owners=$state<RepositoryOwner[]>([]),ownerPage=$state(1),moreOwners=$state(false),ownerWarning=$state(''),ownersLoading=$state(false);
  let expanded=$state<number|null>(null),rows=$state<OnlineRepo[]>([]),page=$state(1),hasMore=$state(false),listLoading=$state(false),listError=$state(''),query=$state('');
  let selected=$state<{accountId:number;repo:OnlineRepo}|null>(null);
  let cloneSource=$state<'account'|'url'>('account'),cloneUrl=$state(''),cloning=$state(false);
  let sequence=0,ownerSequence=0,alive=true;
  const sorted=$derived(accounts.slice().sort((a,b)=>a.login.localeCompare(b.login)));
  const filtered=$derived(rows.filter(r=>r.full_name.toLocaleLowerCase().includes(query.toLocaleLowerCase())));
  onMount(()=>{parent=readablePath(defaultFolder??'');});
  onDestroy(()=>{alive=false;++sequence;++ownerSequence;});
  function choose(next:typeof mode){mode=next;error='';if(next==='create'){accountId=sorted[0]?.id??0;void loadOwners();}if(next==='clone'){cloneSource=sorted.length?'account':'url';name=cloneSource==='url'?cloneFolderName(cloneUrl):'';if(sorted[0])void expand(sorted[0].id);}}
  function changeCloneSource(source:typeof cloneSource){cloneSource=source;error='';name=source==='url'?cloneFolderName(cloneUrl):selected?.repo.name??'';}
  function changeCloneUrl(value:string){if(!name||name===cloneFolderName(cloneUrl))name=cloneFolderName(value);cloneUrl=value;error='';}
  async function browse(field:'folder'|'parent'){if(busy)return;busy=true;error='';try{const path=await api.pickRepoFolder(field==='folder'?'repository':'parent');if(alive&&path!==null){if(field==='folder')folder=path;else parent=path;}}catch(e){if(alive)error=String(e);}finally{busy=false;}}
  async function expand(id:number,next=1){expanded=id;page=next;query='';rows=[];listError='';listLoading=true;selected=null;const request=++sequence;try{const result=await api.onlineRepos(id,next);if(alive&&request===sequence){rows=result.items;hasMore=result.has_more;}}catch(e){if(alive&&request===sequence)listError=String(e);}finally{if(alive&&request===sequence)listLoading=false;}}
  async function loadOwners(next=1){const request=++ownerSequence;const account=accountId;ownersLoading=account!==0;ownerWarning='';if(next===1){owners=[];owner='';}if(!account)return;try{const result=await api.repositoryOwners(account,next);if(alive&&request===ownerSequence){owners=next===1?result.items:[...owners,...result.items.filter(item=>!owners.some(o=>o.login===item.login))];owner=owner||owners[0]?.login||'';ownerPage=next;moreOwners=result.has_more;ownerWarning=result.warning??'';}}catch(e){if(alive&&request===ownerSequence)ownerWarning=String(e);}finally{if(alive&&request===ownerSequence)ownersLoading=false;}}
  async function submit(){if(busy)return;busy=true;error='';try{
    if(mode==='folder'){await oncreated(await api.add(folder.trim()));}
    else if(mode==='clone'){cloning=true;if(cloneSource==='url'){await oncreated(await api.cloneRepositoryUrl(cloneUrl.trim(),parent.trim(),name.trim()));}else if(selected){await oncreated(await api.cloneRepository(selected.accountId,selected.repo.id,parent.trim(),name.trim()));}}
    else if(mode==='create'){const result=await api.createRepository({parent:parent.trim(),name:name.trim(),account_id:accountId||null,owner:accountId?owner:null,private:isPrivate});await oncreated(result.repo,result.warning);}
  }catch(e){if(alive)error=String(e);}finally{busy=false;cloning=false;}}
</script>
<Modal label={mode==='choose'?'Add repository':mode==='clone'?'Clone repository':mode==='create'?'Create repository':'Add from folder'} canClose={!busy} {onclose}>
  <div class="modal-top"><div>{#if mode!=='choose'}<Button class="text-action add-back" disabled={busy} onclick={()=>{mode='choose';error='';++sequence;++ownerSequence;}}><ArrowLeft size={14}/> Back</Button>{/if}<h2>{mode==='choose'?'Add repository':mode==='clone'?'Clone':mode==='create'?'Create repository':'Add from folder'}</h2></div><Button class="top-icon" aria-label="Close dialog" disabled={busy} onclick={onclose}><X size={18}/></Button></div>
  {#if error}<p class="connection-error" role="alert">{error}</p>{/if}
  {#if mode==='choose'}
    <div class="add-choices"><Button onclick={()=>choose('create')}><span><Plus size={23}/></span><div><strong>Create</strong><small>Start a new repository.</small></div></Button><Button onclick={()=>choose('folder')}><span><FolderOpen size={23}/></span><div><strong>Add from folder</strong><small>Open an existing local repository.</small></div></Button><Button onclick={()=>choose('clone')}><span><Download size={23}/></span><div><strong>Clone</strong><small>Download a repository from online.</small></div></Button></div>
  {:else if mode==='folder'}
    <Label for="repo-folder">Repository folder</Label><div class="folder-picker"><Input id="repo-folder" bind:value={folder} placeholder="Choose a repository folder" disabled={busy}/><Button variant="secondary" disabled={busy} onclick={()=>browse('folder')}><FolderOpen size={16}/> Browse…</Button></div><div class="modal-actions"><Button variant="secondary" disabled={busy} onclick={onclose}>Cancel</Button><Button variant="primary" disabled={busy||!folder.trim()} onclick={submit}>{#if busy}<LoaderCircle size={15} class="spin"/>{/if} Add repository</Button></div>
  {:else if mode==='clone'}
    <Tabs.Root bind:value={() => cloneSource, (source) => {
      if (source === 'account' || source === 'url') changeCloneSource(source);
    }}>
    <TabsList class="clone-sources" aria-label="Clone source">
      <TabsTrigger value="account" disabled={busy}>GitHub accounts</TabsTrigger>
      <TabsTrigger value="url" disabled={busy}>Repository URL</TabsTrigger>
    </TabsList>
    {#if cloneSource==='url'}
      <Tabs.Content value="url">
      <Label for="clone-url">Repository URL</Label><Input id="clone-url" value={cloneUrl} oninput={(event)=>changeCloneUrl(event.currentTarget.value)} placeholder="https://github.com/owner/repo.git" spellcheck={false} autocomplete="off" disabled={busy}/><small class="clone-auth-note">Paste an HTTPS or SSH clone URL from your Git host.</small>
      </Tabs.Content>
    {:else}
    <Tabs.Content value="account">
    {#if !sorted.length}<div class="empty-small">Connect a GitHub account to choose a repository.</div><Button variant="primary" onclick={onconnect}>Connect GitHub</Button>{:else}
      <div class="account-repositories">{#each sorted as account (account.id)}<section><Button class="account-group" aria-expanded={expanded===account.id} disabled={busy} onclick={()=>{if(expanded===account.id){expanded=null;++sequence;listLoading=false;}else void expand(account.id);}}><PlatformIcon remote="https://github.com" size={20}/><strong>@{account.login}</strong><ChevronDown size={16}/></Button>{#if expanded===account.id}<div class="account-repo-list"><Input aria-label={'Find repositories for '+account.login} placeholder="Find in this page" bind:value={query} disabled={busy}/>{#if listError}<p class="connection-error" role="alert">{listError}</p><Button class="text-action" onclick={()=>expand(account.id,page)}>Try again</Button>{/if}{#if listLoading}<p role="status" class="empty-small">Loading repositories…</p>{:else}{#each filtered as repo (repo.id)}<Button class={["online-repo", {"is-selected": selected?.accountId===account.id&&selected.repo.id===repo.id}]} disabled={busy} onclick={()=>{selected={accountId:account.id,repo};name=repo.name;}}><span>{repo.full_name}</span>{#if repo.private}<Lock size={13}/>{:else}<Globe size={13}/>{/if}{#if selected?.accountId===account.id&&selected.repo.id===repo.id}<Check size={17}/>{/if}</Button>{:else}{#if !listError}<p class="empty-small">No repositories on this page.</p>{/if}{/each}<div class="history-paging"><Button variant="secondary" disabled={busy||page===1} onclick={()=>expand(account.id,page-1)}>Previous</Button><span>{page}</span><Button variant="secondary" disabled={busy||!hasMore} onclick={()=>expand(account.id,page+1)}>Next</Button></div>{/if}</div>{/if}</section>{/each}</div>
    {/if}
    </Tabs.Content>
    {/if}
    </Tabs.Root>
    {#if cloneSource==='url'||selected}
      {#if cloneSource==='account'&&selected}<p class="selected-clone"><Check size={15}/>{selected.repo.full_name}</p>{/if}
      <Label for="clone-name">Local folder name</Label><Input id="clone-name" bind:value={name} maxlength={100} disabled={busy}/><Label for="clone-parent">Save inside</Label><div class="folder-picker"><Input id="clone-parent" bind:value={parent} placeholder="Choose a parent folder" disabled={busy}/><Button variant="secondary" disabled={busy} onclick={()=>browse('parent')}><FolderOpen size={16}/> Browse…</Button></div><small class="clone-auth-note">{cloneSource==='account'?(selected?.repo.private?'Clone uses the selected GitHub account.':'Public repositories clone without authentication.'):'Private repositories use your system Git credentials or SSH key.'}</small>
    {/if}
    {#if cloning}<p role="status" class="clone-auth-note">Cloning repository… This may take a few minutes.</p>{/if}
    <div class="modal-actions"><Button variant="secondary" disabled={busy} onclick={onclose}>Cancel</Button><Button variant="primary" disabled={busy||(cloneSource==='url'?!cloneUrl.trim():!selected)||!parent.trim()||!name.trim()} onclick={submit}>{#if busy}<LoaderCircle size={15} class="spin"/>{/if} Clone repository</Button></div>
  {:else}
    <Label for="create-name">Repository name</Label><Input id="create-name" bind:value={name} placeholder="my-project" maxlength={100} disabled={busy}/>
    <Label for="create-account">Account</Label><NativeSelect id="create-account" bind:value={accountId} disabled={busy} onchange={()=>loadOwners()}><option value={0}>Local only</option>{#each sorted as account (account.id)}<option value={account.id}>GitHub · @{account.login}</option>{/each}</NativeSelect>
    {#if accountId}<Label for="create-owner">Owner</Label><NativeSelect id="create-owner" bind:value={owner} disabled={busy||ownersLoading}>{#if ownersLoading}<option value="">Loading owners…</option>{/if}{#each owners as item (item.login)}<option value={item.login}>{item.login}{item.type==='Organization'?' · Organization':' · Personal'}</option>{/each}</NativeSelect>{#if moreOwners}<Button class="text-action" disabled={busy||ownersLoading} onclick={()=>loadOwners(ownerPage+1)}>Load more organizations</Button>{/if}{#if ownerWarning}<p class="connection-error" role="alert">{ownerWarning}</p>{/if}<Label for="create-visibility">Visibility</Label><NativeSelect id="create-visibility" bind:value={isPrivate} disabled={busy}><option value={true}>Private</option><option value={false}>Public</option></NativeSelect>{/if}
    <Label for="create-parent">Save inside</Label><div class="folder-picker"><Input id="create-parent" bind:value={parent} placeholder="Choose a parent folder" disabled={busy}/><Button variant="secondary" disabled={busy} onclick={()=>browse('parent')}><FolderOpen size={16}/> Browse…</Button></div>
    <p class="create-summary">{accountId?(isPrivate?'Private':'Public')+' repository · '+(owner||'Choose owner'):'Local repository · main branch'}</p>
    <div class="modal-actions"><Button variant="secondary" disabled={busy} onclick={onclose}>Cancel</Button><Button variant="primary" disabled={busy||!name.trim()||!parent.trim()||(accountId!==0&&(!owner||ownersLoading))} onclick={submit}>{#if busy}<LoaderCircle size={15} class="spin"/>{/if}{accountId?'Create on GitHub':'Create repository'}</Button></div>
  {/if}
</Modal>
