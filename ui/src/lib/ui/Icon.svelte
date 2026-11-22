<script lang="ts" module>
  // 线性图标，24×24 视框，描边 1.8。只收录用到的图标，按需添加。
  export const ICONS = {
    check: 'M5 12.5l4.5 4.5L19 7.5',
    markdown: 'M3.5 6.5h17v11h-17zM6.5 14.5v-5l2.5 3 2.5-3v5M15.5 9.5v5M13.5 12.5l2 2 2-2',
    scan: 'M4 8V5.5A1.5 1.5 0 015.5 4H8M16 4h2.5A1.5 1.5 0 0120 5.5V8M20 16v2.5a1.5 1.5 0 01-1.5 1.5H16M8 20H5.5A1.5 1.5 0 014 18.5V16M8 9.5h8M8 12.5h8M8 15.5h5',
    translate: 'M3 5h9M7.5 3v2M5 5c.5 3.5 3 6.5 6.5 8M10 5c-.8 3.8-3 6.8-6.5 8.5M13 21l4-10 4 10M14.5 17.5h5',
    rect: 'M6 5h12a2 2 0 012 2v10a2 2 0 01-2 2H6a2 2 0 01-2-2V7a2 2 0 012-2z',
    arrow: 'M5 19L19 5M10 5h9v9',
    chevronDown: 'M6 9l6 6 6-6',
    chevronUp: 'M6 15l6-6 6 6',
    copy: 'M9 9h10v10H9zM5 15V5h10',
    close: 'M6 6l12 12M18 6L6 18',
    info: 'M12 8h.01M11 12h1v5h1M12 21a9 9 0 100-18 9 9 0 000 18z',
    alert: 'M12 9v4M12 17h.01M10.3 3.9L2.4 18a2 2 0 001.7 3h15.8a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z',
    pin: 'M9 4h6l-1 5 3 3v2H7v-2l3-3-1-5zM12 14v6',
    download: 'M12 4v11M7.5 10.5L12 15l4.5-4.5M5 19.5h14',
    folder: 'M3.5 7a2 2 0 012-2h4l2 2h7a2 2 0 012 2v8a2 2 0 01-2 2h-13a2 2 0 01-2-2z',
    lock: 'M7 11V8a5 5 0 0110 0v3M6 11h12v9H6z',
    unlock: 'M7 11V8a5 5 0 019.5-2M6 11h12v9H6z',
    plus: 'M12 5v14M5 12h14',
    more: 'M6 12h.01M12 12h.01M18 12h.01',
    pen: 'M4 20l4.5-1L19 8.5 15.5 5 5 15.5z',
    number: 'M12 3.5a8.5 8.5 0 110 17 8.5 8.5 0 010-17zM10.5 9.5L12.5 8v8',
    eraser: 'M8 20h12M5.5 15.5l9-9a2 2 0 012.8 0l1.2 1.2a2 2 0 010 2.8L11 18H8z',
    undo: 'M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 010 11H11',
    refresh: 'M19.5 12a7.5 7.5 0 11-2.2-5.3M19.5 4.5v4h-4',
    textT: 'M5 6h14M12 6v13M9 19h6',
    mosaic: 'M6 4h12a2 2 0 012 2v12a2 2 0 01-2 2H6a2 2 0 01-2-2V6a2 2 0 012-2zM4 12h16M12 4v16',
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
