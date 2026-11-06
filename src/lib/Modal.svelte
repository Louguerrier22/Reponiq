<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  let { label, onclose, children, canClose=true }: { label: string; onclose: () => void; children: Snippet; canClose?: boolean } = $props();
  let dialog: HTMLDialogElement;
  let destroyed=false;
  onMount(() => {
    const trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const overflow=document.body.style.overflow;
    document.body.style.overflow='hidden';
    dialog.showModal();
    return () => { destroyed=true;dialog.close();document.body.style.overflow=overflow;if(trigger?.isConnected)trigger.focus();else document.getElementById('main-content')?.focus({preventScroll:true}); };
  });
</script>

<dialog bind:this={dialog} class="modal" aria-label={label} oncancel={(event)=>{if(!canClose)event.preventDefault();}} onclose={()=>{if(!destroyed)onclose();}}>
  {@render children()}
</dialog>
