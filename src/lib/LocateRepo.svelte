<script lang="ts">
  import Button from './ui/Button.svelte';
  import Checkbox from './ui/Checkbox.svelte';
  import Label from './ui/Label.svelte';
  import Modal from './Modal.svelte';
  import {api,readablePath,type Location,type Repo} from './api';
  let {repo,mainWorktree,onclose,onlocated,onadded}:{repo:Repo;mainWorktree?:string|null;onclose:()=>void;onlocated:()=>Promise<void>;onadded:(repo:Repo)=>Promise<void>}=$props();
  let preview=$state<Location|null>(null),busy=$state(false),acknowledged=$state(false),error=$state('');
  async function inspect(main=false){
    if(busy)return;
    busy=true;error='';acknowledged=false;
    try{
      const path=main?null:await api.pickRepoFolder();
      if(main||path)preview=await api.inspectLocation(repo,path);
    }catch(reason){preview=null;error=String(reason);}
    finally{busy=false;}
  }
  async function apply(){
    if(busy||!preview)return;
    busy=true;error='';
    try{
      if(preview.matching==='mismatch')await onadded(await api.add(preview.path));
      else {await api.relinkRepo(repo,preview.path,acknowledged);await onlocated();}
    }catch(reason){error=String(reason);}
    finally{busy=false;}
  }
</script>
<Modal label="Locate repository folder" canClose={!busy} {onclose}>
  <h2>Locate {repo.name}</h2>
  <p>{readablePath(repo.path)}</p>
  <div class="toolbar">
    <Button variant="secondary" disabled={busy} onclick={()=>inspect()}>Choose Git folder…</Button>
    {#if mainWorktree}<Button variant="secondary" disabled={busy} onclick={()=>inspect(true)}>Use main worktree</Button>{/if}
  </div>
  {#if preview}
    <p><strong>{readablePath(preview.path)}</strong></p>
    <p>Branch {preview.branch} · {preview.remote??'Local repository'}</p>
    {#if preview.matching==='mismatch'}<p>This folder uses a different remote. You can add it as a separate repository.</p>{/if}
    {#if preview.matching==='unknown'}
      <Label class="scope-choice"><Checkbox bind:checked={acknowledged} disabled={busy}/> I checked that this is the correct repository.</Label>
    {/if}
    <div class="modal-actions"><Button variant="primary" disabled={busy||(preview.matching==='unknown'&&!acknowledged)} onclick={apply}>{preview.matching==='mismatch'?'Add as new':'Relink repository'}</Button></div>
  {/if}
  {#if error}<p class="connection-error" role="alert">{error}</p>{/if}
</Modal>
