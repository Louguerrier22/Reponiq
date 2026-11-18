<script lang="ts">
  import { Label } from 'bits-ui';

  let { ref = $bindable(null), ...props }: Label.RootProps = $props();
</script>

<Label.Root bind:ref data-slot="label" {...props}/>
