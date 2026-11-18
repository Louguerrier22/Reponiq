<script lang="ts">
  import { AlertDialog } from 'bits-ui';
  import Button from './Button.svelte';
  import { confirmation, settleConfirmation } from './confirmation.svelte';
</script>

{#if confirmation.current}
  <AlertDialog.Root open={true} onOpenChange={open => { if (!open) settleConfirmation(false); }}>
    <AlertDialog.Portal>
      <div class="ui-dialog-layer">
        <AlertDialog.Overlay class="ui-dialog-overlay" data-slot="dialog-overlay"/>
        <AlertDialog.Content class="modal" data-slot="dialog-content">
          <AlertDialog.Title>{confirmation.current.title}</AlertDialog.Title>
          {#if confirmation.current.description}
            <AlertDialog.Description>{confirmation.current.description}</AlertDialog.Description>
          {/if}
          <div class="modal-actions">
            <AlertDialog.Cancel>
              {#snippet child({ props })}<Button variant="secondary" {...props}>Cancel</Button>{/snippet}
            </AlertDialog.Cancel>
            <AlertDialog.Action onclick={() => settleConfirmation(true)}>
              {#snippet child({ props })}
                <Button variant={confirmation.current?.destructive ? 'destructive' : 'primary'} {...props}>
                  {confirmation.current?.action}
                </Button>
              {/snippet}
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </div>
    </AlertDialog.Portal>
  </AlertDialog.Root>
{/if}
