<script lang="ts">
  import { flip } from '$lib/motion';
  import Icon from './Icon.svelte';
  import { popIn, popOut } from '$lib/motion';
  import { dismiss, toasts } from './toast.svelte';

  const ICON = { ok: 'check', info: 'info', error: 'alert' } as const;
</script>

<div class="host" aria-live="polite">
  {#each toasts as t (t.id)}
    <button
      type="button"
      class="toast {t.kind}"
      in:popIn
      out:popOut
      animate:flip={{ duration: 180 }}
      onclick={() => dismiss(t.id)}
    >
      <span class="badge"><Icon name={ICON[t.kind]} size={13} stroke={2.4} /></span>
      {t.text}
    </button>
  {/each}
</div>

<style>
  .host {
    position: fixed;
    left: 50%;
    bottom: 24px;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column-reverse;
    align-items: center;
    gap: 8px;
    pointer-events: none;
    z-index: 1000;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 14px 0 10px;
    border: 0;
    border-radius: var(--r-panel);
    background: var(--glass);
    backdrop-filter: var(--blur-2);
    box-shadow: var(--elev);
    color: var(--text);
    font-size: 13px;
    white-space: nowrap;
    cursor: default;
  }
  .badge {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--accent);
    color: var(--on-accent);
  }
  .info .badge { background: var(--sunken); color: var(--text2); }
  .error .badge { background: var(--danger); color: #fff; }
</style>
