// 界面多语言。源语言是简体中文：代码里直接写中文，`t('中文')` 查当前语言的词典，查不到就原样显示。
// 词典放在 locales/<语言>.json（键是中文原文，值是译文），只加载当前语言那一份。
//
// - 带变量的句子用 `{name}`：`t('共 {n} 项', { n: 3 })`。
// - 模块顶层的常量表（工具条按钮、快捷键名……）在加载时还不知道语言，只用 `msg('中文')` 标记
//   （原样返回），显示的地方再 `t(label)`。这样切换语言时界面立即跟着变。
// - `t` 读的是 $state，所以放在模板或 $derived 里会随语言自动刷新。

/** 支持的界面语言；名字用各自的母语写法，不随界面语言变化。 */
export const LANGUAGES = [
  { code: 'zh-CN', name: '简体中文' },
  { code: 'zh-TW', name: '繁體中文' },
  { code: 'en', name: 'English' },
  { code: 'ja', name: '日本語' },
  { code: 'ko', name: '한국어' },
  { code: 'es', name: 'Español' },
  { code: 'fr', name: 'Français' },
  { code: 'de', name: 'Deutsch' },
  { code: 'ru', name: 'Русский' },
  { code: 'pt-BR', name: 'Português (Brasil)' },
] as const;

export type Locale = (typeof LANGUAGES)[number]['code'];
/** 设置里存的值：跟随系统，或指定一种语言。 */
export type LanguageSetting = 'system' | Locale;
type Dict = Record<string, string>;

/** 把系统语言标签（zh-Hans-CN、en_US、pt-PT……）对到支持的界面语言；不支持的用英语。 */
export function matchLocale(tag: string): Locale {
  const parts = tag.trim().replace(/_/g, '-').toLowerCase().split('-');
  switch (parts[0]) {
    case 'zh':
      return parts.slice(1).some((p) => ['hant', 'tw', 'hk', 'mo'].includes(p)) ? 'zh-TW' : 'zh-CN';
    case 'ja':
    case 'ko':
    case 'es':
    case 'fr':
    case 'de':
    case 'ru':
      return parts[0];
    case 'pt':
      return 'pt-BR';
    default:
      return 'en';
  }
}

/** 设置值 + 系统语言 → 实际显示的语言。 */
export function resolveLanguage(setting: LanguageSetting, systemTag: string): Locale {
  return setting === 'system' ? matchLocale(systemTag) : setting;
}

const loaders: Record<Locale, (() => Promise<{ default: Dict }>) | null> = {
  'zh-CN': null, // 源语言，不需要词典
  'zh-TW': () => import('../locales/zh-TW.json'),
  en: () => import('../locales/en.json'),
  ja: () => import('../locales/ja.json'),
  ko: () => import('../locales/ko.json'),
  es: () => import('../locales/es.json'),
  fr: () => import('../locales/fr.json'),
  de: () => import('../locales/de.json'),
  ru: () => import('../locales/ru.json'),
  'pt-BR': () => import('../locales/pt-BR.json'),
};

const store = $state<{ locale: Locale; dict: Dict }>({ locale: 'zh-CN', dict: {} });
let ticket = 0;

/** 当前界面语言。 */
export const currentLocale = (): Locale => store.locale;

/** 切换界面语言：先加载词典，加载完一次性换掉，界面不会闪出半中半英。 */
export async function setLocale(locale: Locale): Promise<void> {
  if (locale === store.locale && (locale === 'zh-CN' || Object.keys(store.dict).length)) return;
  const mine = ++ticket;
  const load = loaders[locale];
  let dict: Dict = {};
  if (load) {
    try {
      dict = (await load()).default;
    } catch {
      dict = {}; // 词典读不到就显示中文原文，不让界面起不来
    }
  }
  if (mine !== ticket) return; // 期间又换了别的语言
  store.dict = dict;
  store.locale = locale;
  if (typeof document !== 'undefined') document.documentElement.lang = locale;
}

/** 用 params 替换 `{name}`；没有对应参数的占位符原样保留。 */
export function format(text: string, params?: Record<string, unknown>): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m));
}

/** 翻译：key 是中文原文。 */
export function t(key: string, params?: Record<string, unknown>): string {
  return format(store.dict[key] ?? key, params);
}

/** 标记「这是要翻译的文字」，原样返回；用在模块顶层的常量里，显示时再 `t()`。 */
export const msg = (key: string): string => key;
