<script lang="ts">
  import Button from './ui/Button.svelte';
  import Checkbox from './ui/Checkbox.svelte';
  import Input from './ui/Input.svelte';
  import Label from './ui/Label.svelte';
 import {FolderOpen,LoaderCircle} from 'lucide-svelte';
 import {api,readablePath,type WorkspaceSettings} from './api';
 let {settings,initial=false,busy=$bindable(false),onsaved}:{settings:WorkspaceSettings;initial?:boolean;busy?:boolean;onsaved:(settings:WorkspaceSettings)=>void}=$props();
 let folder=$state(''),detect=$state(true),error=$state(''),saved=$state(false);
 $effect(()=>{folder=readablePath(settings.default_folder??'');detect=initial?true:settings.auto_detect;});
 async function browse(){if(busy)return;busy=true;error='';try{const path=await api.pickRepoFolder('parent');if(path!==null){folder=path;saved=false;}}catch(e){error=String(e);}finally{busy=false;}}
 async function save(skip=false){if(busy)return;busy=true;error='';try{const value=await api.saveWorkspaceSettings({default_folder:skip?null:folder.trim()||null,auto_detect:!skip&&detect,setup_complete:true});saved=true;onsaved(value);}catch(e){error=String(e);}finally{busy=false;}}
</script>
<div class="workspace-folder">
 <Label for={initial?'setup-folder':'settings-folder'}>Default repository folder</Label><div class="folder-picker"><Input id={initial?'setup-folder':'settings-folder'} placeholder="Choose where your repositories live" bind:value={folder} disabled={busy} oninput={()=>saved=false}/><Button variant="secondary" disabled={busy} onclick={browse}><FolderOpen size={16}/> Browse…</Button></div>
 <Label class="scope-choice"><Checkbox bind:checked={detect} disabled={busy||!folder.trim()}/><span>Automatically find repositories in this folder</span></Label><p class="folder-help">Review discovered repositories before adding them.</p>
 {#if error}<p class="connection-error" role="alert">{error}</p>{/if}{#if saved&&!initial}<p class="folder-saved" role="status">Folder saved</p>{/if}
 <div class="modal-actions">{#if initial}<Button class="text-action" disabled={busy} onclick={()=>save(true)}>Skip for now</Button>{/if}<Button variant="primary" disabled={busy||(initial&&!folder.trim())} onclick={()=>save()}>{#if busy}<LoaderCircle class="spin" size={15}/>{/if}{initial?'Save & continue':'Save folder'}</Button></div>
</div>
