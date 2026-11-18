<script lang="ts">
  import Button from './ui/Button.svelte';
  import Checkbox from './ui/Checkbox.svelte';
  import Label from './ui/Label.svelte';
  import NativeSelect from './ui/NativeSelect.svelte';
  import {onMount} from 'svelte';
  import {api, type FetchSettings} from './api';

  let {initial = false, busy = $bindable(false), ondone}: {
    initial?: boolean; busy?: boolean; ondone?: () => void;
  } = $props();
  let fetchSettings = $state<FetchSettings | null>(null);
  let autostart = $state<boolean | null>(null);
  let startAtSignIn = $state(false);
  let unavailable = $state('');
  let error = $state('');
  let saved = $state(false);
  let alive = true;

  onMount(() => {
    void api.fetchSettings().then(value => {
      if (alive) fetchSettings = value;
    }).catch(reason => {if (alive) error = String(reason);});
    void readAutostart();
    return () => {alive = false;};
  });

  async function readAutostart() {
    try {
      const value = await api.autostartStatus();
      if (!alive) return;
      autostart = startAtSignIn = value;
      unavailable = '';
    } catch (reason) {
      if (alive) {autostart = null; unavailable = String(reason);}
    }
  }

  async function save() {
    if (!fetchSettings || busy) return;
    busy = true;
    error = '';
    saved = false;
    try {
      fetchSettings = await api.saveFetchSettings({...fetchSettings});
      if (autostart !== null && (!startAtSignIn || startAtSignIn !== autostart)) {
        const desired = startAtSignIn;
        autostart = startAtSignIn = await api.setAutostart(desired);
        if (autostart !== desired) throw new Error('Your system did not apply the sign-in startup setting.');
      }
      saved = true;
      ondone?.();
    } catch (reason) {
      error = String(reason);
      await readAutostart();
    } finally {
      busy = false;
    }
  }
</script>

<div class="startup-options">
{#if fetchSettings}
  <Label class="scope-choice"><Checkbox bind:checked={fetchSettings.enabled} disabled={busy} onCheckedChange={()=>saved=false}/> Fetch on launch</Label>
  <p class="folder-help">Refreshes remote branches in the background. You can start working immediately.</p>
  <Label for="launch-fetch-scope">Repositories to fetch</Label>
  <NativeSelect id="launch-fetch-scope" bind:value={fetchSettings.scope} disabled={busy || !fetchSettings.enabled} onchange={()=>saved=false}>
    <option value="pinned">Pinned repositories</option>
    <option value="all">All registered repositories</option>
  </NativeSelect>
{/if}
<Label class="scope-choice"><Checkbox bind:checked={startAtSignIn} disabled={busy || autostart === null} onCheckedChange={()=>saved=false}/> Start Reponiq at sign-in</Label>
{#if unavailable}<p class="folder-help">{unavailable}</p>{/if}
{#if error}<p class="connection-error" role="alert">{error}</p>{/if}
{#if saved && !initial}<p class="folder-saved" role="status">Startup preferences saved.</p>{/if}
<div class="modal-actions">
  {#if initial}<Button class="text-action" disabled={busy} onclick={ondone}>Skip for now</Button>{/if}
  <Button variant="primary" disabled={busy || !fetchSettings} onclick={save}>{initial ? 'Save & finish' : 'Save startup preferences'}</Button>
</div>
</div>
