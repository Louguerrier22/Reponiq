<script lang="ts">
  import { Tabs } from 'bits-ui';

  let { ...props }: Tabs.TriggerProps = $props();
</script>

<Tabs.Trigger data-slot="tabs-trigger" {...props}/>
