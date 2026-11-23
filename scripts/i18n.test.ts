import { describe, expect, it } from 'vitest';
import { LOCALES, checkLocales } from './i18n.mjs';

// 界面和后端用到的每一句中文都要有每种语言的译文，占位符要对得上，没用到的旧键要清掉。
// 缺了怎么补：node scripts/i18n.mjs todo 列出缺的键，翻译后 node scripts/i18n.mjs apply 合并。
describe('多语言词典', () => {
  const { keys, problems } = checkLocales();

  it('找得到要翻译的句子', () => {
    expect(keys.ui.size).toBeGreaterThan(300);
    expect(keys.rs.size).toBeGreaterThan(50);
    expect(LOCALES).toHaveLength(9);
  });

  it('每种语言都有译文', () => {
    expect(problems.missing).toEqual([]);
  });

  it('译文的 {占位符} 和原文一致', () => {
    expect(problems.mismatch).toEqual([]);
  });

  it('没有用不到的旧键', () => {
    expect(problems.stale).toEqual([]);
  });
});
