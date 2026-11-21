import '../../tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { startConfig } from '$lib/config.svelte';
import { installErrorLog } from '$lib/log';

installErrorLog();

await startConfig();
mount(App, { target: document.getElementById('app')! });
