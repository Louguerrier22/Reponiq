<script lang="ts">
  import Button from './ui/Button.svelte';
  import Modal from './Modal.svelte';
  import { onMount, tick } from 'svelte';
  import { api } from './api';
  let {onclose,prepare,hasRepo=false}:{onclose:()=>void;prepare:(target:string)=>Promise<void>;hasRepo?:boolean}=$props();
  let step=$state(0),busy=$state(false),saving=$state(false),error=$state('');
  let heading:HTMLHeadingElement;let alive=true,frame=0;
  let ring=$state({left:0,top:0,width:0,height:0});let card=$state({left:20,top:20});
  const steps=$derived([
    {title:'Add your first repository.',body:'Choose Create, Add from folder or Clone. Select where the repository lives on your computer.',target:'add'},
    {title:'Find a project quickly.',body:'Search by name, branch, tag or collection. Ctrl+K brings you straight here.',target:'search'},
    ...(hasRepo?[{title:'Review and commit.',body:'Open a file for its diff. Choose which changes to include, write a message here, then commit before pushing.',target:'commit'}]:[]),
    {title:'Connect GitHub when ready.',body:'Local Git works without an account. Connect here for pull requests and workflow status.',target:'connect'}
  ]);
  const current=$derived(steps[step]);
  function position(){const el=document.querySelector<HTMLElement>('[data-tour="'+current.target+'"]');if(!el)return;const b=el.getBoundingClientRect();ring={left:Math.max(6,b.left-7),top:Math.max(6,b.top-7),width:Math.min(b.width+14,innerWidth-12),height:b.height+14};const width=Math.min(370,innerWidth-32);const below=b.bottom+24;card={left:Math.max(16,Math.min(b.left,innerWidth-width-16)),top:below+245<innerHeight?below:Math.max(16,b.top-265)};}
  async function move(next:number){if(busy)return;busy=true;step=next;error='';try{await prepare(current.target);await tick();if(!alive)return;document.querySelector<HTMLElement>('[data-tour="'+current.target+'"]')?.scrollIntoView({block:'center',behavior:'instant'});position();heading?.focus();}catch{error='This part of the tour is unavailable. You can skip it.';}finally{busy=false;}}
  async function finish(){if(saving)return;saving=true;error='';try{await api.finishWalkthrough();onclose();}catch{error='Could not save your preference. Try again.';}finally{saving=false;}}
  onMount(()=>{document.body.classList.add('tour-active');void move(0);const reposition=()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(position);};window.addEventListener('resize',reposition);document.addEventListener('scroll',reposition,true);return()=>{alive=false;cancelAnimationFrame(frame);window.removeEventListener('resize',reposition);document.removeEventListener('scroll',reposition,true);document.body.classList.remove('tour-active');};});
</script>
<Modal label="Reponiq walkthrough" onclose={finish} canClose={!saving} walkthrough>
  <div class="tour-ring" aria-hidden="true" style:left={ring.left+'px'} style:top={ring.top+'px'} style:width={ring.width+'px'} style:height={ring.height+'px'}></div>
  <section class="tour-card" style:left={card.left+'px'} style:top={card.top+'px'}>
    <p class="tour-progress" aria-live="polite">{step+1} / {steps.length}</p>
    <h2 bind:this={heading} tabindex="-1">{current.title}</h2><p>{current.body}</p>
    {#if error}<p role="alert" class="connection-error">{error}</p>{/if}
    <div class="tour-actions"><Button class="text-action" disabled={saving} onclick={finish}>Skip tour</Button><div>{#if step>0}<Button variant="secondary" disabled={busy||saving} onclick={()=>move(step-1)}>Back</Button>{/if}<Button variant="primary" disabled={busy||saving} onclick={()=>step===steps.length-1?finish():move(step+1)}>{step===steps.length-1?'Get started':'Next'}</Button></div></div>
  </section>
</Modal>
