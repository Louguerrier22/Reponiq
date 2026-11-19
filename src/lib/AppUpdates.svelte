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
    let quitRequest: number | undefined;
    let frozenDialogs: HTMLDialogElement[] = [];
    function resumeAfterQuit(request: number) {
      if (quitRequest !== request) return;
      quitRequest = undefined;
      for (const dialog of frozenDialogs) dialog.inert = false;
      frozenDialogs = [];
      document.body.inert = false;
    }
    const subscription = listen<string>('app-update-error', event => {
      error = event.payload;
      installing = false;
      open = true;
      if (quitRequest !== undefined) resumeAfterQuit(quitRequest);
    }).catch(() => undefined);
    const quitTimeout = listen<number>('app-quit-timed-out', ({ payload: request }) => {
      if (quitRequest !== request) return;
      resumeAfterQuit(request);
      error = 'Saving drafts took too long. Reponiq is still open. Try quitting again.';
      open = true;
    }).catch(() => undefined);
    const quitSubscription = listen<number>('app-quit-requested', async ({ payload: request }) => {
      if (quitRequest !== undefined) resumeAfterQuit(quitRequest);
      quitRequest = request;
      document.body.inert = true;
      // Native modal dialogs escape inertness inherited from the document body.
      frozenDialogs = [...document.querySelectorAll<HTMLDialogElement>('dialog[open]:not([inert])')];
      for (const dialog of frozenDialogs) dialog.inert = true;
      let draftsSaved = false;
      let accepted = false;
      try {
        try {
          await flushDraftWrites();
          draftsSaved = true;
        } catch (reason) {
          error = `Could not save the latest draft. ${String(reason)}`;
          open = true;
        }
        accepted = await invoke<boolean>('finish_app_quit', { request, draftsSaved });
      } catch (reason) {
        error = String(reason);
        open = true;
      } finally {
        if (!accepted || !draftsSaved) resumeAfterQuit(request);
      }
    }).then(async unlisten => {
      try {
        await invoke('register_app_quit_handler');
        return unlisten;
      } catch (reason) {
        unlisten();
        throw reason;
      }
    }).catch(() => undefined);
    void check();
    return () => {
      if (quitRequest !== undefined) resumeAfterQuit(quitRequest);
      void subscription.then(unlisten => unlisten?.());
      void quitTimeout.then(unlisten => unlisten?.());
      void quitSubscription.then(unlisten => unlisten?.());
    };
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
