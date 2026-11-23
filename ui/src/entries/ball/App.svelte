<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 悬浮球（需求第 11 节、M10.5）：单击截图，右键菜单，按住拖动（交给系统窗口拖动，完全跟手），
  // 松手后由 Rust 贴到最近的边、一半藏起来；指针移上来滑出，移开 300 ms 后缩回。
  import { ballClick, ballMenu, ballPeek, inTauri } from '$lib/ipc';

  let press: { x: number; y: number } | null = null;
  let pressed = $state(false);
  let leaveTimer: ReturnType<typeof setTimeout> | undefined;

  async function startDrag() {
    if (!inTauri()) return;
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    getCurrentWindow().startDragging();
  }

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    press = { x: e.screenX, y: e.screenY };
    pressed = true;
  }

  function move(e: PointerEvent) {
    if (!press) return;
    if (Math.hypot(e.screenX - press.x, e.screenY - press.y) > 3) {
      press = null;
      pressed = false;
      startDrag(); // 系统接管拖动，松手时页面收不到 pointerup
    }
  }

  function up(e: PointerEvent) {
    if (e.button !== 0) return;
    const click = press !== null;
    press = null;
    pressed = false;
    if (click) ballClick();
  }

  function enter() {
    clearTimeout(leaveTimer);
    ballPeek(true);
  }

  function leave() {
    press = null;
    pressed = false;
    clearTimeout(leaveTimer);
    leaveTimer = setTimeout(() => ballPeek(false), 300);
  }
</script>

<button
  type="button"
  class="ball"
  class:pressed
  aria-label={t('截图（右键更多）')}
  title={t('截图（右键更多）')}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointerenter={enter}
  onpointerleave={leave}
  oncontextmenu={(e) => {
    e.preventDefault();
    ballMenu();
  }}
>
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <path d="M4 8V5.5A1.5 1.5 0 015.5 4H8M16 4h2.5A1.5 1.5 0 0120 5.5V8M20 16v2.5a1.5 1.5 0 01-1.5 1.5H16M8 20H5.5A1.5 1.5 0 014 18.5V16M8 10h8M8 14h5" />
  </svg>
</button>

<style>
  :global(html), :global(body) {
    margin: 0;
    height: 100%;
    background: transparent;
    overflow: hidden;
    user-select: none;
  }
  :global(#app) { height: 100%; display: flex; align-items: center; justify-content: center; }
  .ball {
    width: 44px;
    height: 44px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--glass2);
    backdrop-filter: var(--blur-1);
    box-shadow: var(--elev-sm);
    color: var(--accent);
    cursor: pointer;
    transition:
      transform var(--d-hover) var(--ease-hover),
      box-shadow var(--d-hover) var(--ease-hover),
      background var(--d-hover) var(--ease-hover);
  }
  .ball:hover { transform: scale(1.06); box-shadow: var(--elev); }
  .ball.pressed { transform: scale(.92); }
  svg { width: 22px; height: 22px; fill: none; stroke: currentColor; stroke-width: 1.9; stroke-linecap: round; stroke-linejoin: round; }
</style>
