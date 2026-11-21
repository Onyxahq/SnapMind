<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';

  type Props = HTMLInputAttributes & {
    value?: string | number;
    /** 等宽字体（路径、模板、快捷键） */
    mono?: boolean;
    width?: string;
  };
  let { value = $bindable(''), mono = false, width, type = 'text', ...rest }: Props = $props();
</script>

<input {type} class="field" class:mono bind:value style:width {...rest} />

<style>
  .field {
    height: 34px;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line2);
    background: var(--field);
    color: var(--text);
    font-size: 13px;
    padding: 0 10px;
    min-width: 0;
    transition:
      border-color var(--d-hover) var(--ease-hover),
      box-shadow var(--d-hover) var(--ease-hover);
  }
  .field::placeholder { color: var(--text3); }
  .field:hover:not(:focus):not(:disabled) { border-color: var(--text3); }
  .field:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .field:disabled { opacity: .5; cursor: not-allowed; }
  .mono { font-family: var(--font-mono); font-size: 12.5px; }
</style>
