<script lang="ts">
  import Badge from './ui/Badge.svelte';
  import Button from './ui/Button.svelte';
  import Input from './ui/Input.svelte';
  import Label from './ui/Label.svelte';
  import { X } from 'lucide-svelte';
  import {tick} from 'svelte';
  let {label,items=$bindable<string[]>([]),maxItems=20,maxLength=50}: {label:string;items?:string[];maxItems?:number;maxLength?:number}=$props();
  let draft=$state('');
  let input = $state<HTMLInputElement>();
  const id=$props.id();
  function add(){const value=draft.trim();if(value&&items.length<maxItems&&!items.some(item=>item.toLocaleLowerCase()===value.toLocaleLowerCase()))items=[...items,value];draft='';}
  async function remove(item:string){items=items.filter(value=>value!==item);await tick();input?.focus();}
</script>
<div class="token-field"><Label for={id}>{label}</Label><div class="token-input"><div class="tokens">{#each items as item}<Badge class="token-chip"><span>{item}</span><Button type="button" aria-label={'Remove '+label.toLowerCase()+' '+item} onclick={()=>remove(item)}><X size={13}/></Button></Badge>{/each}</div><Input {id} bind:ref={input} aria-label={label} placeholder={items.length>=maxItems?'':'Type and press Enter'} maxlength={maxLength} disabled={items.length>=maxItems} bind:value={draft} onkeydown={(event)=>{if(event.key==='Enter'&&!event.isComposing){event.preventDefault();add();}}} onblur={add}/></div></div>
