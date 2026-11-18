<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements';

  let { children, ...props }: HTMLAttributes<HTMLSpanElement> = $props();
</script>

<span data-slot="badge" {...props}>
  {@render children?.()}
</span>
