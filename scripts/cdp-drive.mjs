// 真机自动化测试：通过 WebView2 远程调试端口（Chrome DevTools Protocol）驱动截图遮罩页面。
// 不依赖系统鼠标（系统级弹窗、变暗层挡住模拟鼠标时也能测）。
//
// 1. 用远程调试端口启动调试版（PowerShell）：
//      $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9333'
//      Start-Process target\debug\snapmind.exe --autostart -RedirectStandardError err.txt -RedirectStandardOutput out.txt
//    （stdout / stderr 都要重定向，否则子进程继承管道，调用方会一直等它退出）
// 2. 按 Shift+F1 开始截图，然后：node scripts/cdp-drive.mjs 9333 steps.json
//
// steps.json：[{ "op": "drag", "x1", "y1", "x2", "y2" } | { "op": "click", "x", "y" }
//   | { "op": "key", "key", "mods"(1 Alt / 2 Ctrl / 4 Meta / 8 Shift) } | { "op": "type", "text" }
//   | { "op": "wait", "ms" } | { "op": "eval", "expr" }]，坐标是遮罩窗口内的 CSS 像素。
import { readFileSync } from 'node:fs';

const [port, scriptPath] = process.argv.slice(2);
const steps = JSON.parse(readFileSync(scriptPath, 'utf8'));
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = targets.find((t) => t.url.includes('/overlay/'));
if (!page) throw new Error('overlay page not found');

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let id = 0;
const pending = new Map();
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
};
const send = (method, params = {}) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function mouse(type, x, y, buttons = 0) {
  await send('Input.dispatchMouseEvent', { type, x, y, button: type === 'mouseMoved' && !buttons ? 'none' : 'left', buttons, clickCount: 1 });
}
const KEYS = { Escape: 27, Enter: 13, ' ': 32 };
async function key(k, mods = 0) {
  const code = KEYS[k] ?? k.toUpperCase().charCodeAt(0);
  const text = k.length === 1 && !mods ? k : undefined;
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: k, windowsVirtualKeyCode: code, modifiers: mods, text });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: k, windowsVirtualKeyCode: code, modifiers: mods });
}

for (const s of steps) {
  switch (s.op) {
    case 'wait': await sleep(s.ms); break;
    case 'click':
      await mouse('mouseMoved', s.x, s.y); await mouse('mousePressed', s.x, s.y, 1); await mouse('mouseReleased', s.x, s.y); break;
    case 'drag': {
      await mouse('mouseMoved', s.x1, s.y1); await mouse('mousePressed', s.x1, s.y1, 1);
      for (let i = 1; i <= 10; i++) await mouse('mouseMoved', s.x1 + ((s.x2 - s.x1) * i) / 10, s.y1 + ((s.y2 - s.y1) * i) / 10, 1);
      await mouse('mouseReleased', s.x2, s.y2); break;
    }
    case 'key': await key(s.key, s.mods ?? 0); break;
    case 'type': await send('Input.insertText', { text: s.text }); break;
    case 'eval': {
      const r = await send('Runtime.evaluate', { expression: s.expr, awaitPromise: true, returnByValue: true });
      console.log('eval:', JSON.stringify(r.result?.result?.value ?? r.result));
      break;
    }
  }
  await sleep(60);
}
ws.close();
