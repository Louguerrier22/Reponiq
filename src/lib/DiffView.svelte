<script lang="ts">
  let {code,label='Diff'}:{code:string;label?:string}=$props();
  const lines=$derived(code.slice(0,200001).split('\n',1501));
  const limited=$derived(code.length>200000||lines.length>1500);
  function tone(line:string){return line.startsWith('@@')?'diff-hunk':line.startsWith('+')&&!line.startsWith('+++')?'diff-add':line.startsWith('-')&&!line.startsWith('---')?'diff-remove':'diff-context';}
</script>
<!-- svelte-ignore a11y_no_noninteractive_tabindex (Focusable, read-only scroll region.) -->
<div class="diff-code" role="region" tabindex="0" aria-label={label}><pre><code>{#each lines.slice(0,1500) as line,index}<span class={tone(line)}><i aria-hidden="true">{index+1}</i>{line||' '}</span>{/each}</code></pre></div>
{#if limited}<p class="diff-limit">Preview limited to 1,500 lines / 200,000 characters.</p>{/if}
