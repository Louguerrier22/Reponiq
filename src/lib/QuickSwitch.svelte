<script lang="ts">
  import Button from './ui/Button.svelte';
  import Input from './ui/Input.svelte';
  import Label from './ui/Label.svelte';
  import Modal from './Modal.svelte';
  import {type Repo,type RepoWorkflow} from './api';
  let {repos,metadata,onselect,onclose,onmove,onlocation,canOpen}: {
    repos:Repo[];metadata:Map<number,RepoWorkflow>;onselect:(repo:Repo)=>void;
    onclose:()=>void;onmove:(repo:Repo,earlier:boolean)=>Promise<void>;
    onlocation:(repo:Repo,action:'editor'|'terminal'|'folder')=>void;canOpen:boolean;
  }=$props();
  let query=$state(''),index=$state(0),moving=$state(false);
  const matching=$derived(repos.filter(repo=>[repo.name,repo.path,repo.branch,repo.collection??'',...repo.tags]
    .some(text=>text.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()))));
  const pinned=$derived(matching.filter(repo=>repo.pinned));
  const recent=$derived(matching.filter(repo=>!repo.pinned&&metadata.get(repo.id)?.opened_at)
    .sort((a,b)=>(metadata.get(b.id)?.opened_at??0)-(metadata.get(a.id)?.opened_at??0)).slice(0,8));
  const results=$derived((query.trim()?matching:[...pinned,...recent,...matching.filter(repo=>!repo.pinned&&!recent.includes(repo))]).slice(0,60));
  const selected=$derived(results[index]??results[0]);
  function keys(event:KeyboardEvent){
    if(!results.length)return;
    if(event.key==='ArrowDown'||event.key==='ArrowUp'){
      event.preventDefault();index=(index+(event.key==='ArrowDown'?1:-1)+results.length)%results.length;
      document.getElementById('repo-result-'+results[index].id)?.scrollIntoView({block:'nearest'});
    }else if(event.key==='Enter'&&selected){event.preventDefault();onselect(selected);}
  }
  async function move(repo:Repo,earlier:boolean){moving=true;try{await onmove(repo,earlier);}finally{moving=false;}}
</script>
<Modal label="Switch repository" {onclose}>
  <h2>Switch repository</h2>
  <Label for="quick-repo-search">Find a repository</Label>
  <Input id="quick-repo-search" type="search" bind:value={query} oninput={()=>index=0} onkeydown={keys} placeholder="Name, folder, branch, or tag"/>
  <div class="branch-results">
    {#each results as repo,i (repo.id)}
      <div class="quick-repo-row">
        <Button id={'repo-result-'+repo.id} class={["commit-choice", {"is-selected": index===i}]} onclick={()=>onselect(repo)}>
          <span><strong>{repo.name}</strong><small>{repo.pinned?'Pinned':recent.includes(repo)?'Recent':'Repository'} · {repo.branch||'Branch unavailable'}</small></span>
        </Button>
        {#if repo.pinned&&!query.trim()}
          <div class="toolbar">
            <Button class="text-action" aria-label={'Move '+repo.name+' up'} disabled={moving||i===0} onclick={()=>move(repo,true)}>↑</Button>
            <Button class="text-action" aria-label={'Move '+repo.name+' down'} disabled={moving||i===pinned.length-1} onclick={()=>move(repo,false)}>↓</Button>
          </div>
        {/if}
      </div>
    {:else}<p>No matching repositories.</p>{/each}
    {#if matching.length>60}<p>Showing 60 results. Search to narrow the list.</p>{/if}
  </div>
  {#if selected && canOpen}
    <div class="modal-actions">
      <Button variant="secondary" onclick={()=>onlocation(selected,'editor')}>Open in editor</Button>
      <Button variant="secondary" onclick={()=>onlocation(selected,'terminal')}>Open terminal</Button>
      <Button variant="secondary" onclick={()=>onlocation(selected,'folder')}>Open folder</Button>
    </div>
  {/if}
</Modal>
