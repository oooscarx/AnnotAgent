import { setLocale, t, useLocale, type Locale } from "../i18n";

export function LanguageSelector() {
  const locale = useLocale();
  return <label className="language-selector">
    <span>{t("Language")}</span>
    <select aria-label="Language / 语言" value={locale} onChange={(event) => setLocale(event.target.value as Locale)}>
      <option value="en" lang="en">English</option>
      <option value="zh-CN" lang="zh-CN">简体中文</option>
    </select>
  </label>;
}
