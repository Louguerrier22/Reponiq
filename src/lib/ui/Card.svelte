<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements';

  type Props = HTMLAttributes<HTMLElement> & { as?: 'article' | 'div' | 'section' };
  let { as = 'div', children, ...props }: Props = $props();
</script>

<svelte:element this={as} data-slot="card" {...props}>
  {@render children?.()}
</svelte:element>
