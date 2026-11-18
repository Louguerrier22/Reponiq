<script lang="ts">
  import { Collapsible } from 'bits-ui';
  import type { Snippet } from 'svelte';

  let {
    open = $bindable(false), heading, children, class: classes
  }: { open?: boolean; heading: Snippet; children: Snippet; class?: string } = $props();
</script>

<Collapsible.Root bind:open class={classes} data-slot="collapsible">
  <Collapsible.Trigger class="ui-disclosure-trigger" data-slot="collapsible-trigger">
    <span class="ui-disclosure-marker" aria-hidden="true">{open ? '▾' : '▸'}</span>
    {@render heading()}
  </Collapsible.Trigger>
  <Collapsible.Content class="ui-disclosure-content" data-slot="collapsible-content">
    {@render children()}
  </Collapsible.Content>
</Collapsible.Root>
