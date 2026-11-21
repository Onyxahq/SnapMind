<script lang="ts">
  // 强调色色点。显示的颜色跟随当前明暗（深夜下是提亮版本）。
  import { ACCENTS, type Accent } from '$lib/theme';

  let {
    value = $bindable(),
    onchange,
  }: { value: Accent; onchange?: (v: Accent) => void } = $props();

  function pick(v: Accent) {
    if (v === value) return;
    value = v;
    onchange?.(v);
  }
</script>

<div class="dots" role="radiogroup" aria-label="强调色">
  {#each ACCENTS as a (a.id)}
    <button
      type="button"
      role="radio"
      aria-checked={value === a.id}
      aria-label={a.label}
      title={a.label}
      class="dot"
      class:on={value === a.id}
      style:--c-light={a.light}
      style:--c-dark={a.dark}
      onclick={() => pick(a.id)}
    ></button>
  {/each}
</div>

<style>
  .dots { display: flex; gap: 12px; }
  .dot {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 0;
    padding: 0;
    cursor: pointer;
    background: var(--c-light);
    transition:
      transform var(--d-hover) var(--ease-switch),
      box-shadow var(--d-hover) var(--ease-hover);
  }
  :global([data-theme='dark']) .dot { background: var(--c-dark); }
  .dot:hover { transform: scale(1.1); }
  .dot:active { transform: scale(.92); }
  .dot.on { box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px var(--text); }
  .dot:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px var(--accent); }
</style>
