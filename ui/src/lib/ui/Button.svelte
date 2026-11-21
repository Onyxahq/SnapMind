<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import Icon, { type IconName } from './Icon.svelte';

  type Props = HTMLButtonAttributes & {
    /** primary：强调色实心；ghost：玻璃描边；danger：描边红字 */
    variant?: 'primary' | 'ghost' | 'danger';
    icon?: IconName;
    children?: Snippet;
  };
  let { variant = 'ghost', icon, children, type = 'button', ...rest }: Props = $props();
</script>

<button {type} class="btn {variant}" {...rest}>
  {#if icon}<Icon name={icon} />{/if}
  {@render children?.()}
</button>

<style>
  .btn {
    height: 32px;
    padding: 0 12px;
    border-radius: var(--r-ctl);
    font-size: 13px;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    flex: none;
    white-space: nowrap;
    transition:
      background-color var(--d-hover) var(--ease-hover),
      filter var(--d-hover) var(--ease-hover),
      transform var(--d-press) var(--ease-press);
  }
  .btn:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .primary {
    padding: 0 14px;
    border: 0;
    font-weight: 600;
    background: var(--accent);
    color: var(--on-accent);
  }
  .primary:hover { filter: brightness(1.08); }
  .primary:active { transform: scale(.95); }
  .ghost,
  .danger {
    border: 1px solid var(--line2);
    background: var(--field);
    color: var(--text);
  }
  .danger { color: var(--danger); }
  .ghost:hover,
  .danger:hover { background: var(--hover); }
  .ghost:active,
  .danger:active { transform: scale(.96); }
  .btn:disabled {
    opacity: .45;
    cursor: not-allowed;
    transform: none;
    filter: none;
  }
</style>
