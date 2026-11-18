<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { type Snippet, untrack } from 'svelte';
  import DialogContent from './ui/DialogContent.svelte';

  let {
    label, onclose, children, canClose = true, walkthrough = false
  }: {
    label: string; onclose: () => void; children: Snippet;
    canClose?: boolean; walkthrough?: boolean;
  } = $props();
  const opener = untrack(() => document.activeElement instanceof HTMLElement ? document.activeElement : null);

  function restoreFocus(event: Event) {
    event.preventDefault();
    const target = opener?.isConnected ? opener : document.getElementById('main-content');
    target?.focus({ preventScroll: true });
  }
</script>

<Dialog.Root open={true} onOpenChange={(open) => { if (!open) onclose(); }}>
  <DialogContent
    class={walkthrough ? 'tour-overlay' : 'modal'}
    overlayClass={walkthrough ? 'ui-dialog-overlay tour-backdrop' : 'ui-dialog-overlay'}
    aria-label={label}
    onInteractOutside={(event) => event.preventDefault()}
    onEscapeKeydown={(event) => {
      if (!canClose || walkthrough) event.preventDefault();
      if (walkthrough && canClose) onclose();
    }}
    onCloseAutoFocus={restoreFocus}
  >
    {@render children()}
  </DialogContent>
</Dialog.Root>
