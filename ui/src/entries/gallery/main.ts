import '../../tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { applyTheme } from '$lib/theme';

applyTheme('light', 'blue');
mount(App, { target: document.getElementById('app')! });
