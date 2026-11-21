<script lang="ts" module>
  // 线性图标，24×24 视框，描边 1.8。只收录用到的图标，按需添加。
  export const ICONS = {
    check: 'M5 12.5l4.5 4.5L19 7.5',
    translate: 'M3 5h9M7.5 3v2M5 5c.5 3.5 3 6.5 6.5 8M10 5c-.8 3.8-3 6.8-6.5 8.5M13 21l4-10 4 10M14.5 17.5h5',
    rect: 'M6 5h12a2 2 0 012 2v10a2 2 0 01-2 2H6a2 2 0 01-2-2V7a2 2 0 012-2z',
    arrow: 'M5 19L19 5M10 5h9v9',
    chevronDown: 'M6 9l6 6 6-6',
    copy: 'M9 9h10v10H9zM5 15V5h10',
    close: 'M6 6l12 12M18 6L6 18',
    info: 'M12 8h.01M11 12h1v5h1M12 21a9 9 0 100-18 9 9 0 000 18z',
    alert: 'M12 9v4M12 17h.01M10.3 3.9L2.4 18a2 2 0 001.7 3h15.8a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z',
  } as const;
  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  let { name, size = 17, stroke = 1.8 }: { name: IconName; size?: number; stroke?: number } = $props();
</script>

<svg
  class="ic"
  viewBox="0 0 24 24"
  width={size}
  height={size}
  stroke-width={stroke}
  aria-hidden="true"
><path d={ICONS[name]} /></svg>

<style>
  .ic {
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    flex: none;
  }
</style>
