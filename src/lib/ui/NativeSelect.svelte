<script lang="ts" generics="Value">
  import type { HTMLSelectAttributes } from 'svelte/elements';

  type Props = Omit<HTMLSelectAttributes, 'value'> & { value?: Value; ref?: HTMLSelectElement };
  let { ref = $bindable(), value = $bindable(), children, ...props }: Props = $props();
</script>

<select bind:this={ref} bind:value data-slot="native-select" {...props}>
  {@render children?.()}
</select>
