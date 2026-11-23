<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 有未保存的标注时退出前确认（需求 3.1），可以勾选「不再询问」。
  // Enter 放弃并退出，Esc 取消（回到截图）。
  import { onMount } from 'svelte';
  import Button from '$lib/ui/Button.svelte';
  import { popIn, popOut } from '$lib/motion';

  let {
    count,
    onconfirm,
    oncancel,
  }: { count: number; onconfirm: (dontAsk: boolean) => void; oncancel: () => void } = $props();

  let dontAsk = $state(false);
  let box: HTMLDivElement;

  onMount(() => box.focus());

  function onKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter') {
      e.preventDefault();
      onconfirm(dontAsk);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      oncancel();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onpointerdown={(e) => e.stopPropagation()} onkeydown={onKey}>
  <div class="box" role="alertdialog" aria-modal="true" aria-label={t('放弃标注')} tabindex="-1" bind:this={box} in:popIn out:popOut>
    <b>{t('放弃这次截图的标注？')}</b>
    <p>{t('{count} 处标注还没有复制或保存，退出后会丢失。', { count })}</p>
    <label class="ask"><input type="checkbox" bind:checked={dontAsk} />{t('不再询问')}</label>
    <div class="actions">
      <Button onclick={oncancel}>{t('取消')}</Button>
      <Button variant="danger" onclick={() => onconfirm(dontAsk)}>{t('放弃并退出 Enter')}</Button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, .25);
    z-index: 60;
    cursor: default;
  }
  .box {
    width: 340px;
    padding: 18px 18px 14px;
    border-radius: var(--r-card);
    background: var(--glass2);
    backdrop-filter: var(--blur-2);
    box-shadow: var(--elev);
    color: var(--text);
    display: flex;
    flex-direction: column;
    gap: 8px;
    outline: none;
  }
  b { font-size: 14px; }
  p { margin: 0; font-size: 13px; line-height: 20px; color: var(--text2); }
  .ask {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
    font-size: 12.5px;
    color: var(--text2);
    cursor: pointer;
  }
  .ask input { accent-color: var(--accent); width: 15px; height: 15px; margin: 0; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 8px; }
</style>
