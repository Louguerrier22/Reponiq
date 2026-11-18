<script lang="ts">
  import { Checkbox } from 'bits-ui';

  let {
    ref = $bindable(null), checked = $bindable(false), class: classes, ...props
  }: Omit<Checkbox.RootProps, 'children' | 'child'> = $props();
</script>

<Checkbox.Root bind:ref bind:checked class={['ui-checkbox', classes]} data-slot="checkbox" {...props}/>
