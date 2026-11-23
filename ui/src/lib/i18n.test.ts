import { describe, expect, it } from 'vitest';
import { LANGUAGES, format, matchLocale, resolveLanguage } from './i18n.svelte';

describe('matchLocale', () => {
  it('把系统语言标签对到支持的界面语言', () => {
    expect(matchLocale('zh-CN')).toBe('zh-CN');
    expect(matchLocale('zh-Hans-CN')).toBe('zh-CN');
    expect(matchLocale('zh')).toBe('zh-CN');
    expect(matchLocale('zh-TW')).toBe('zh-TW');
    expect(matchLocale('zh-Hant')).toBe('zh-TW');
    expect(matchLocale('zh-HK')).toBe('zh-TW');
    expect(matchLocale('en_US')).toBe('en');
    expect(matchLocale('ja-JP')).toBe('ja');
    expect(matchLocale('pt-PT')).toBe('pt-BR');
    expect(matchLocale('DE-at')).toBe('de');
  });

  it('不支持的语言和空值用英语', () => {
    expect(matchLocale('ar-EG')).toBe('en');
    expect(matchLocale('')).toBe('en');
  });

  it('每种支持的语言自己都能对回自己', () => {
    for (const l of LANGUAGES) expect(matchLocale(l.code)).toBe(l.code);
  });
});

describe('resolveLanguage', () => {
  it('跟随系统时看系统语言，否则用指定的', () => {
    expect(resolveLanguage('system', 'fr-FR')).toBe('fr');
    expect(resolveLanguage('system', 'xx')).toBe('en');
    expect(resolveLanguage('ko', 'fr-FR')).toBe('ko');
  });
});

describe('format', () => {
  it('替换占位符，缺参数的保留原样', () => {
    expect(format('共 {n} 项', { n: 3 })).toBe('共 3 项');
    expect(format('{a} → {b}', { a: 1 })).toBe('1 → {b}');
  });
  it('没有参数时原样返回（提示词里的 {text} 之类不会被动）', () => {
    expect(format('用 {target_lang}')).toBe('用 {target_lang}');
  });
});
