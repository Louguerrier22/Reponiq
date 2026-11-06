<script lang="ts">
  import { X } from 'lucide-svelte';
  import {tick} from 'svelte';
  let {label,items=$bindable<string[]>([]),maxItems=20,maxLength=50}: {label:string;items?:string[];maxItems?:number;maxLength?:number}=$props();
  let draft=$state('');
  let input: HTMLInputElement;
  const id=$props.id();
  function add(){const value=draft.trim();if(value&&items.length<maxItems&&!items.some(item=>item.toLocaleLowerCase()===value.toLocaleLowerCase()))items=[...items,value];draft='';}
  async function remove(item:string){items=items.filter(value=>value!==item);await tick();input?.focus();}
</script>
<div class="token-field"><label for={id}>{label}</label><div class="token-input"><div class="tokens">{#each items as item}<span class="token-chip"><span>{item}</span><button type="button" aria-label={'Remove '+label.toLowerCase()+' '+item} onclick={()=>remove(item)}><X size={13}/></button></span>{/each}</div><input {id} bind:this={input} aria-label={label} placeholder={items.length>=maxItems?'':'Type and press Enter'} maxlength={maxLength} disabled={items.length>=maxItems} bind:value={draft} onkeydown={(event)=>{if(event.key==='Enter'&&!event.isComposing){event.preventDefault();add();}}} onblur={add}/></div></div>
