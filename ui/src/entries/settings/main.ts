import '../../tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { startConfig } from '$lib/config.svelte';
import { installErrorLog } from '$lib/log';

installErrorLog();

// 先读到配置并上好主题再挂载，避免先闪一下默认主题。
await startConfig();
mount(App, { target: document.getElementById('app')! });
