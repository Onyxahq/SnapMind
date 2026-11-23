<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 文字标注的编辑框：放在文字所在位置，字号颜色与最终效果一致，随内容自动变宽变高。
  // 回车换行；Esc、Ctrl+Enter 或点别处（失去焦点）提交。
  import { onMount } from 'svelte';
  import { TEXT_LINE_HEIGHT, textFont, type TextShape } from '$lib/annotate';

  let {
    shape,
    scale,
    oninput,
    oncommit,
  }: {
    shape: TextShape;
    /** 显示器缩放（物理像素 / CSS 像素） */
    scale: number;
    oninput: (text: string) => void;
    oncommit: () => void;
  } = $props();

  let el: HTMLTextAreaElement;
  const fontPx = $derived(shape.width / scale);

  onMount(() => {
    // 下一帧再聚焦：避开触发创建的那次鼠标事件带来的焦点变化
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(el.value.length, el.value.length);
    });
  });

  function onKey(e: KeyboardEvent) {
    e.stopPropagation(); // 编辑态的快捷键（1–7、WASD、空格…）不能在打字时生效
    if (e.key === 'Escape' || (e.key === 'Enter' && (e.ctrlKey || e.metaKey))) {
      e.preventDefault();
      el.blur();
    }
  }
</script>

<textarea
  bind:this={el}
  class="ted"
  style:left="{shape.at.x / scale}px"
  style:top="{shape.at.y / scale}px"
  style:font={textFont(fontPx)}
  style:line-height={TEXT_LINE_HEIGHT}
  style:color={shape.color}
  value={shape.text}
  spellcheck="false"
  rows="1"
  aria-label={t('文字标注')}
  oninput={(e) => oninput(e.currentTarget.value)}
  onkeydown={onKey}
  onblur={oncommit}
  onpointerdown={(e) => e.stopPropagation()}
></textarea>

<style>
  .ted {
    position: absolute;
    margin: 0;
    padding: 0;
    border: 0;
    outline: 1px dashed var(--accent);
    outline-offset: 3px;
    background: transparent;
    resize: none;
    overflow: hidden;
    white-space: pre;
    /* 随内容自动伸缩 */
    field-sizing: content;
    min-width: 1ch;
    caret-color: var(--accent);
    z-index: 25;
  }
</style>
