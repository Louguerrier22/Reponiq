<script lang="ts">
  import type { HTMLTextareaAttributes } from 'svelte/elements';

  type Props = HTMLTextareaAttributes & { ref?: HTMLTextAreaElement };
  let { ref = $bindable(), value = $bindable(), ...props }: Props = $props();
</script>

<textarea bind:this={ref} bind:value data-slot="textarea" {...props}></textarea>
