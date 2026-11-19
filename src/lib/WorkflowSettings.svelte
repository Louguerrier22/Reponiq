<script lang="ts">
  import Button from './ui/Button.svelte';
  import Collapsible from './ui/Collapsible.svelte';
  import { confirm } from './ui/confirmation.svelte';
  import Checkbox from './ui/Checkbox.svelte';
  import Input from './ui/Input.svelte';
  import Label from './ui/Label.svelte';
  import NativeSelect from './ui/NativeSelect.svelte';
  import Textarea from './ui/Textarea.svelte';
  import { untrack } from 'svelte';
  import {api,readablePath,type AppPreferences,type RepoPreferences,type RepoContext,type Target} from './api';
  let {target=null,app,preferences=null,context=null,onsaved,onidentity=()=>{}}: {
    target?:Target|null;app:AppPreferences;preferences?:RepoPreferences|null;context?:RepoContext|null;
    onsaved:(value:AppPreferences|RepoPreferences)=>void;onidentity?:(value:RepoContext)=>void;
  }=$props();
  const defaults:RepoPreferences={fetch_mode:'inherit',warn_default:'inherit',editor:null,review_paths:[],preferred_base:null};
  let global=$state<AppPreferences>(untrack(()=>({...app})));
  let local=$state<RepoPreferences>(untrack(()=>({...defaults,...preferences})));
  let paths=$state(untrack(()=>local.review_paths.join('\n')));
  let author=$state(untrack(()=>context?.local_name??''));
  let email=$state(untrack(()=>context?.local_email??''));
  let busy=$state(false),message=$state(''),error=$state('');
  let executable=$derived(target ? local.editor : global.editor);
  $effect(() => {
    if (context) {
      author = context.local_name ?? '';
      email = context.local_email ?? '';
    }
  });

  async function browse(){
    if(busy)return;
    busy=true;error='';
    try {
      const path=await api.pickEditor();
      if(path){if(target)local.editor=path;else global.editor=path;}
    } catch(reason){error=String(reason);}
    finally{busy=false;}
  }
  async function save(){
    if(busy)return;
    if(!target&&app.save_drafts&&!global.save_drafts&&!await confirm({
      title:'Delete saved drafts on this device?',
      description:'Open drafts stay available until you close Reponiq.',
      action:'Delete saved drafts',
      destructive:true
    }))return;
    busy=true;error='';message='';
    try{
      if(target){
        const saved=await api.saveRepoWorkflow(target,{...local,preferred_base:local.preferred_base?.trim()||null,
          review_paths:[...new Set(paths.split(/\r?\n/).filter(path=>path.trim()))]});
        local=saved;onsaved(saved);
      }else{
        global=await api.saveWorkflowApp({...global});onsaved(global);
      }
      message='Preferences saved.';
    }catch(reason){error=String(reason);}
    finally{busy=false;}
  }
  async function saveIdentity(useDefault:boolean){
    if(!target||busy)return;
    busy=true;error='';message='';
    try{
      const next=await api.setIdentity(target,useDefault?null:{name:author.trim(),email:email.trim()});
      author=next.local_name??'';email=next.local_email??'';
      onidentity(next);message=next.author ? 'Git author is '+next.author : 'Git has no author configured.';
    }catch(reason){error=String(reason);}
    finally{busy=false;}
  }
</script>

<div class="workflow-settings-form">
  {#if target}
    <Label>Fetch on launch
      <NativeSelect bind:value={local.fetch_mode} disabled={busy}>
        <option value="inherit">Follow startup scope</option><option value="on">Include at launch</option><option value="off">Manual only</option>
      </NativeSelect>
    </Label>
    <Label>Default-branch reminder
      <NativeSelect bind:value={local.warn_default} disabled={busy}>
        <option value="inherit">Use app default</option><option value="on">Always warn</option><option value="off">Off</option>
      </NativeSelect>
    </Label>
    <Label>Preferred PR base<Input bind:value={local.preferred_base} placeholder="Use repository default" maxlength={200} disabled={busy}/></Label>
    <Label>Collapse review paths<Textarea bind:value={paths} placeholder={'One file or folder per line\npackage-lock.json\ngenerated/'} rows={3} disabled={busy}></Textarea></Label>
  {:else}
    <Label class="scope-choice"><Checkbox bind:checked={global.save_drafts} disabled={busy}/> Save drafts on this device</Label>
    <Label class="scope-choice"><Checkbox bind:checked={global.warn_default} disabled={busy}/> Remind me before committing to the default branch</Label>
  {/if}
  <Label>{target?'Repository editor':'Default editor'}
    <Input value={executable?readablePath(executable):''} placeholder={target?'Use app default':'Choose an installed editor'} readonly/>
  </Label>
  <div class="toolbar">
    <Button variant="secondary" onclick={browse} disabled={busy}>Choose editor…</Button>
    {#if executable}<Button class="text-action" disabled={busy} onclick={()=>{if(target)local.editor=null;else global.editor=null;}}>{target?'Use app default':'Clear editor'}</Button>{/if}
    <Button variant="primary" onclick={save} disabled={busy}>Save preferences</Button>
  </div>
  {#if target && context}
    <Collapsible class="workflow-identity" open={!context.author}>
      {#snippet heading()}Git commit author{/snippet}
      <p>{context.author??'Git has no author configured.'}</p>
      <Label>Name<Input bind:value={author} disabled={busy} maxlength={200}/></Label>
      <Label>Email<Input type="email" bind:value={email} disabled={busy} maxlength={254}/></Label>
      <div class="toolbar">
        <Button variant="secondary" onclick={()=>saveIdentity(false)} disabled={busy||!author.trim()||!email.trim()}>Set repository author</Button>
        <Button class="text-action" onclick={()=>saveIdentity(true)} disabled={busy}>Use Git default</Button>
      </div>
    </Collapsible>
  {/if}
  {#if message}<p class="inline-success" role="status">{message}</p>{/if}
  {#if error}<p class="connection-error" role="alert">{error}</p>{/if}
</div>
