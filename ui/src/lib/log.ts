// 前端日志转发到 Rust（stderr；M11.2 起写入滚动日志文件）。
// 遮罩等隐藏窗口没法开开发者工具，未捕获的错误也要能看到。
import { invoke } from '@tauri-apps/api/core';
import { inTauri } from './ipc';

export type Level = 'info' | 'warn' | 'error';

export function log(level: Level, msg: string): void {
  if (!inTauri()) {
    console[level === 'info' ? 'log' : level](msg);
    return;
  }
  invoke('ui_log', { level, msg: `[${location.pathname}] ${msg}` }).catch(() => {});
}

/** 捕获未处理的错误和 Promise 拒绝。每个窗口入口调用一次。 */
export function installErrorLog(): void {
  addEventListener('error', (e) => log('error', `${e.message} @ ${e.filename}:${e.lineno}`));
  addEventListener('unhandledrejection', (e) => log('error', `未处理的拒绝：${String(e.reason)}`));
}
