<script lang="ts">
  import { FolderGit2, GitBranch } from 'lucide-svelte';
  import { remoteInfo } from './platform';
  import github from '../../assets/platforms/github.svg';
  import bitbucket from '../../assets/platforms/bitbucket.svg';
  import codeberg from '../../assets/platforms/codeberg.svg';
  import gitea from '../../assets/platforms/gitea.svg';
  let { remote, size=24 }: { remote: string | null; size?: number } = $props();
  const info=$derived(remoteInfo(remote));
  const logos: Record<string,string>={github,bitbucket,codeberg,gitea};
</script>
<span class="platform-icon" title={info.label} style:width="{size}px" style:height="{size}px">
  {#if logos[info.platform]}<img class:round-mark={info.platform==='github'||info.platform==='codeberg'} src={logos[info.platform]} width={size} height={size} alt={info.label}/>
  {:else if info.platform==='local'}<span role="img" aria-label={info.label}><FolderGit2 {size}/></span>
  {:else}<span role="img" aria-label={info.label}><GitBranch {size}/></span>{/if}
</span>
