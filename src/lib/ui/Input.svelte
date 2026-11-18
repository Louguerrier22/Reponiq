<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';

  type Props = HTMLInputAttributes & { ref?: HTMLInputElement };
  let { ref = $bindable(), value = $bindable(), ...props }: Props = $props();
</script>

<input bind:this={ref} bind:value data-slot="input" {...props}/>
