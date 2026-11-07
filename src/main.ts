import { mount } from 'svelte';
import App from './App.svelte';
import './style.css';
import { captureLog, flushLogs, startDiagnostics } from './lib/diagnostics';
const stopDiagnostics = startDiagnostics();
if(import.meta.hot)import.meta.hot.dispose(stopDiagnostics);
mount(App, { target: document.getElementById('app')! });
requestAnimationFrame(()=>requestAnimationFrame(()=>{
  captureLog('info','app','Frontend mounted');
  void flushLogs();
}));
