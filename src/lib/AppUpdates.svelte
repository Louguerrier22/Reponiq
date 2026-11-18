<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { ArrowDownToLine, RotateCw } from 'lucide-svelte';
  import { invoke } from './diagnostics';
  import { flushDraftWrites } from './draft-store';
  import Button from './ui/Button.svelte';
  import Modal from './Modal.svelte';

  type Status = { state: 'current' | 'checking' | 'unavailable' } | { state: 'ready'; version: string };
  let { busy }: { busy: boolean } = $props();
  let status = $state<Status>({ state: 'checking' });
  let checking = $state(false);
  let installing = $state(false);
  let open = $state(false);
  let error = $state('');

  async function check() {
    if (checking || installing || status.state === 'ready') return;
    checking = true;
    error = '';
    try {
      status = await invoke<Status>('check_app_update');
    } catch (reason) {
      error = String(reason);
    } finally {
      checking = false;
    }
  }

  async function restart() {
    if (busy || installing || status.state !== 'ready') return;
    installing = true;
    error = '';
    try {
      await flushDraftWrites();
      await invoke('restart_for_update');
    } catch (reason) {
      error = String(reason);
      installing = false;
    }
  }

  onMount(() => {
    const subscription = listen<string>('app-update-error', event => {
      error = event.payload;
      installing = false;
      open = true;
    }).catch(() => undefined);
    void check();
    return () => { void subscription.then(unlisten => unlisten?.()); };
  });
</script>

<Button
  class={['app-updates', { 'update-ready': status.state === 'ready' }]}
  title={status.state === 'ready' ? 'Update ready' : 'App updates'}
  onclick={() => open = true}
>
  <ArrowDownToLine size={17}/> <span>{status.state === 'ready' ? 'Update ready' : 'App updates'}</span>
</Button>

{#if open}
  <Modal label="App updates" canClose={!installing} onclose={() => open = false}>
    <h2>App updates</h2>
    {#if error}<p class="connection-error" role="alert">{error}</p>{/if}
    {#if installing}
      <p role="status">Installing the update…</p>
    {:else if status.state === 'ready'}
      <p>Reponiq {status.version} is ready. Restart now or fully quit Reponiq to apply the update.</p>
    {:else if checking}
      <p role="status">Checking for and preparing updates…</p>
    {:else if status.state === 'unavailable'}
      <p>This installation uses manual updates. Download the latest installer or update through your package manager.</p>
    {:else if !error}
      <p>No updates available.</p>
    {/if}
    <div class="modal-actions">
      <Button variant="secondary" disabled={installing} onclick={() => open = false}>Close</Button>
      {#if status.state === 'ready'}
        <Button variant="primary" disabled={busy || installing} onclick={restart}><RotateCw size={15}/> Restart and update</Button>
      {:else if status.state !== 'unavailable'}
        <Button variant="primary" disabled={checking} onclick={check}>Check for updates</Button>
      {/if}
    </div>
  </Modal>
{/if}
