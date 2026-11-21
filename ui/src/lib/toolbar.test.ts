import { describe, expect, it } from 'vitest';
import { formatBytes, joinPath, lockedSize, placeToolbar } from './toolbar';

const VIEW = { width: 1600, height: 900 };
const BAR = { width: 200, height: 38 };

describe('placeToolbar', () => {
  it('默认在选区下方右对齐', () => {
    expect(placeToolbar({ x: 400, y: 200, width: 300, height: 200 }, BAR, VIEW)).toEqual({ x: 500, y: 408 });
  });
  it('下方放不下就放上方', () => {
    expect(placeToolbar({ x: 400, y: 500, width: 300, height: 380 }, BAR, VIEW)).toEqual({ x: 500, y: 454 });
  });
  it('上下都放不下时放进选区内右下角', () => {
    expect(placeToolbar({ x: 0, y: 0, width: 1600, height: 900 }, BAR, VIEW)).toEqual({ x: 1392, y: 854 });
  });
  it('选区太靠左时不超出屏幕', () => {
    expect(placeToolbar({ x: 0, y: 100, width: 50, height: 50 }, BAR, VIEW).x).toBe(8);
  });
});

describe('保存窗口小工具', () => {
  it('文件大小', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(2048)).toBe('2.0 KB');
    expect(formatBytes(300 * 1024)).toBe('300 KB');
    expect(formatBytes(3 * 1024 * 1024)).toBe('3.00 MB');
  });
  it('锁定比例', () => {
    const orig = { width: 1920, height: 1080 };
    expect(lockedSize(orig, 'width', 960)).toEqual({ width: 960, height: 540 });
    expect(lockedSize(orig, 'height', 720)).toEqual({ width: 1280, height: 720 });
    expect(lockedSize(orig, 'width', 0)).toEqual({ width: 1, height: 1 });
  });
  it('拼路径', () => {
    expect(joinPath('C:\\Pics\\', 'a', 'png')).toBe('C:\\Pics\\a.png');
    expect(joinPath('D:/x', 'b', 'jpg')).toBe('D:/x\\b.jpg');
  });
});
