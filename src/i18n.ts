// 軽量 i18n。外部ライブラリ非依存。
// 言語追加は locales/xx.json を足して dicts / LOCALES に1行加えるだけ。

import ja from "./locales/ja.json";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";

export type Locale = "ja" | "en" | "zh-CN";

type Dict = Record<string, string>;
const dicts: Record<Locale, Dict> = {
  ja: ja as Dict,
  en: en as Dict,
  "zh-CN": zhCN as Dict,
};

// 言語セレクタ用の表示名（各言語自身の表記）。
export const LOCALES: { code: Locale; label: string }[] = [
  { code: "ja", label: "日本語" },
  { code: "en", label: "English" },
  { code: "zh-CN", label: "简体中文" },
];

let current: Locale = "ja";

export function getLocale(): Locale {
  return current;
}

export function isLocale(v: unknown): v is Locale {
  return v === "ja" || v === "en" || v === "zh-CN";
}

/** 文字列を取得。未定義キーは en → キー名の順でフォールバック。{name} を params で置換。 */
export function t(key: string, params?: Record<string, string | number>): string {
  let s = dicts[current][key] ?? dicts.en[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      s = s.split(`{${k}}`).join(String(v));
    }
  }
  return s;
}

/** data-i18n / data-i18n-placeholder / data-i18n-title を走査して現在言語を適用。 */
export function applyDom(root: ParentNode = document): void {
  root.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => {
    el.textContent = t(el.dataset.i18n as string);
  });
  root.querySelectorAll<HTMLElement>("[data-i18n-placeholder]").forEach((el) => {
    (el as HTMLInputElement).placeholder = t(el.dataset.i18nPlaceholder as string);
  });
  root.querySelectorAll<HTMLElement>("[data-i18n-title]").forEach((el) => {
    el.title = t(el.dataset.i18nTitle as string);
  });
  document.documentElement.lang = current;
}

/** 言語を切り替えて静的 DOM を再適用する。 */
export function setLocale(loc: Locale): void {
  current = loc;
  applyDom();
}

/** ブラウザ言語から初期言語を推定（ja / zh* / それ以外は en）。 */
export function detectLocale(): Locale {
  const n = (navigator.language || "en").toLowerCase();
  if (n.startsWith("ja")) return "ja";
  if (n.startsWith("zh")) return "zh-CN";
  return "en";
}
