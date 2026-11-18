<script lang="ts">
  import { Tabs } from 'bits-ui';

  let { class: classes, ...props }: Tabs.ListProps = $props();
</script>

<Tabs.List class={['tabs', classes]} data-slot="tabs-list" {...props}/>
