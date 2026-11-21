import '../../tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { applyTheme } from '$lib/theme';

// M0.4 起从配置读取；这里先用默认值（跟随系统、蓝色）。
applyTheme('system', 'blue');
mount(App, { target: document.getElementById('app')! });
