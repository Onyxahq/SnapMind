<script lang="ts">
  import { flip } from '$lib/motion';
  import Icon from './Icon.svelte';
  import { popIn, popOut } from '$lib/motion';
  import { dismiss, toasts } from './toast.svelte';

  const ICON = { ok: 'check', info: 'info', error: 'alert' } as const;
</script>

<div class="host" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast {t.kind}" role="status" in:popIn out:popOut animate:flip={{ duration: 180 }}>
      <button type="button" class="body" onclick={() => dismiss(t.id)}>
        <span class="badge"><Icon name={ICON[t.kind]} size={13} stroke={2.4} /></span>
        {t.text}
      </button>
      {#if t.action}
        <button
          type="button"
          class="act"
          onclick={() => {
            t.action?.run();
            dismiss(t.id);
          }}>{t.action.label}</button>
      {/if}
    </div>
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
    height: 40px;
    padding: 0 14px 0 10px;
    border-radius: var(--r-panel);
    background: var(--glass);
    backdrop-filter: var(--blur-2);
    box-shadow: var(--elev);
    color: var(--text);
    font-size: 13px;
    white-space: nowrap;
  }
  .body {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 100%;
    padding: 0;
    border: 0;
    background: none;
    cursor: default;
  }
  .act {
    margin: 0 -8px 0 10px;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--r-tag);
    background: none;
    color: var(--accent);
    font-weight: 600;
    cursor: pointer;
    transition: background-color var(--d-hover) var(--ease-hover);
  }
  .act:hover { background: var(--accent-soft); }
  .act:focus-visible { outline: 2px solid var(--accent); }
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
