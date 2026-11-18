<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';

  type Props = HTMLButtonAttributes & {
    ref?: HTMLButtonElement;
    variant?: 'default' | 'primary' | 'secondary' | 'ghost' | 'destructive';
  };
  let { ref = $bindable(), variant = 'default', type = 'button', children, class: classes, ...props }: Props = $props();
</script>

<button bind:this={ref} {type} class={[variant !== 'default' && variant, classes]} data-slot="button" {...props}>
  {@render children?.()}
</button>
