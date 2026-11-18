<script lang="ts">
  import Button from './ui/Button.svelte';
  import Input from './ui/Input.svelte';
  import Label from './ui/Label.svelte';
  import type { Branch, RepoDetails } from './api';
  import Modal from './Modal.svelte';

  let {details, busy, error, recentBranches=[],defaultBranch=null,onclose, onswitch, onfetch, oncreate, onrestore,onopenworktree}: {
    details: RepoDetails;
    busy: boolean;
    error: string;
    recentBranches?:string[];
    defaultBranch?:string|null;
    onclose: () => void;
    onswitch: (branch: Branch, strategy: string) => Promise<boolean>;
    onfetch: () => Promise<boolean>;
    oncreate: () => void;
    onrestore: (oid: string) => Promise<boolean>;
    onopenworktree:(reference:string)=>void;
  } = $props();
  let query = $state('');
  let target = $state<Branch | null>(null);
  let visibleCount = $state(60);
  const rank=(branch:Branch)=>{
    if(branch.remote)return 100;
    if(branch.name===details.git.branch)return -2;
    if(branch.name===defaultBranch)return -1;
    const recent=recentBranches.indexOf(branch.name);
    return recent===-1?90:recent;
  };
  let filtered = $derived(details.branches.filter(branch =>
    (branch.remote ? branch.remote + '/' + branch.name : branch.name)
      .toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())).sort((a,b)=>rank(a)-rank(b)));
  const blocked = $derived(!!details.git.operation || details.git.conflicts.length > 0);

  async function choose(branch: Branch) {
    if (details.git.dirty) target = branch;
    else if (await onswitch(branch, 'clean')) onclose();
  }
  async function switchTarget(strategy: string) {
    if (target && await onswitch(target, strategy)) onclose();
    else target = null;
  }
</script>

<Modal label="Switch branch" canClose={!busy} {onclose}>
  {#if error}<p class="connection-error" role="alert">{error}</p>{/if}
  {#if busy}<p role="status">Waiting for Git…</p>{/if}
  {#if target}
    <h2>Switch to {target.name}?</h2>
    <p>{details.git.staged} staged · {details.changes.filter(file=>file.x!=='?'&&file.y!==' '&&!details.git.conflicts.includes(file.path)).length} unstaged · {details.changes.filter(file=>file.x==='?').length} new paths.</p>
    <p>A file can contain both staged and unstaged edits. Choose where to keep your work.</p>
    <div class="branch-decisions">
      <Button variant="primary" disabled={busy || blocked || !details.git.has_head || details.git.branch === '(detached)'} onclick={() => switchTarget('stash')}>Stash and switch</Button>
      <p>Save your edits here and restore them when you return.</p>
      <Button variant="secondary" disabled={busy || blocked} onclick={() => switchTarget('bring')}>Bring changes</Button>
      <p>Take your edits to the selected branch if Git can do so safely.</p>
      <Button class="text-action" disabled={busy} onclick={onclose}>Stay here</Button>
    </div>
  {:else}
    <h2>Switch branch</h2>
    <Label for="branch-search">Find a branch</Label>
    <Input id="branch-search" type="search" placeholder="Search local and fetched branches" bind:value={query} oninput={() => visibleCount = 60}/>
    {#if blocked}<p>Resolve conflicts or finish the current Git operation before switching.</p>{/if}
    <div class="branch-results">
      {#each filtered.slice(0, visibleCount) as branch (branch.reference)}
        {@const current = !branch.remote && branch.name === details.git.branch}
        <div class="quick-repo-row">
          <Button class={["commit-choice", {"is-selected": current}]} aria-current={current ? 'true' : undefined} disabled={busy || current || blocked || branch.in_worktree} onclick={() => choose(branch)}>
            <span><strong>{branch.name}</strong><small>{current ? 'Current branch' : branch.remote ? 'Remote · ' + branch.remote : branch.in_worktree?'Open in another worktree':branch.name===defaultBranch?'Default branch':'Local branch'}</small></span>
          </Button>
          {#if !current&&branch.in_worktree}<Button variant="secondary" disabled={busy} onclick={()=>onopenworktree(branch.reference)}>Open worktree</Button>{/if}
        </div>
      {:else}
        <p>No matching branches. Fetch to check for updates from the remote.</p>
      {/each}
      {#if filtered.length > visibleCount}<Button variant="secondary" onclick={() => visibleCount += 60}>Show more branches</Button>{/if}
    </div>
    {#if details.saved_changes.length}
      <h3>Saved changes on this branch</h3>
      <p>Restore into a clean working folder. Git keeps a recovery copy.</p>
      <div class="branch-results">
        {#each details.saved_changes as entry, index (entry.oid)}
          <Button variant="secondary" disabled={busy || blocked || !!details.git.dirty} onclick={async () => {if (await onrestore(entry.oid)) onclose();}}>
            Restore changes · {new Date(entry.timestamp * 1000).toLocaleString()} · {entry.oid.slice(0, 7)}{index === 0 ? ' (latest)' : ''}
          </Button>
        {/each}
      </div>
    {/if}
    <div class="modal-actions">
      <Button class="text-action" disabled={busy} onclick={onclose}>Close</Button>
      <Button variant="secondary" disabled={busy || !details.git.remotes.length} onclick={onfetch}>Fetch branches</Button>
      <Button variant="primary" disabled={busy || blocked} onclick={oncreate}>Create branch…</Button>
    </div>
  {/if}
</Modal>
