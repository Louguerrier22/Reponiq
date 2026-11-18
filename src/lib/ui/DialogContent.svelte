<script lang="ts">
  import { Dialog } from 'bits-ui';

  type Props = Dialog.ContentProps & { overlayClass?: string };
  let { ref = $bindable(null), overlayClass = 'ui-dialog-overlay', ...props }: Props = $props();
</script>

<Dialog.Portal>
  <div class="ui-dialog-layer">
    <Dialog.Overlay class={overlayClass} data-slot="dialog-overlay"/>
    <Dialog.Content bind:ref data-slot="dialog-content" {...props}/>
  </div>
</Dialog.Portal>
