import { useSyncExternalStore } from "react";
import { zhCN } from "./locales/zh-CN";

export type Locale = "en" | "zh-CN";
export const LOCALE_STORAGE_KEY = "annotagent.locale";
const listeners = new Set<() => void>();

export function resolveLocale(saved: string | null, languages: readonly string[]): Locale {
  if (saved === "en" || saved === "zh-CN") return saved;
  return languages[0]?.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

function initialLocale(): Locale {
  if (typeof window === "undefined") return "en";
  let saved: string | null = null;
  try { saved = window.localStorage.getItem(LOCALE_STORAGE_KEY); } catch { /* Session preference still works. */ }
  return resolveLocale(saved, navigator.languages ?? [navigator.language]);
}

let locale = initialLocale();
function publish(next: Locale) {
  locale = next;
  if (typeof document !== "undefined") document.documentElement.lang = next;
  listeners.forEach((listener) => listener());
}

export function setLocale(next: Locale) {
  if (next !== "en" && next !== "zh-CN") return;
  try { window.localStorage.setItem(LOCALE_STORAGE_KEY, next); } catch { /* Keep the in-memory preference. */ }
  publish(next);
}

if (typeof window !== "undefined") {
  document.documentElement.lang = locale;
  window.addEventListener("storage", (event) => {
    if (event.key === LOCALE_STORAGE_KEY || event.key === null)
      publish(resolveLocale(event.newValue, navigator.languages ?? [navigator.language]));
  });
}

export function useLocale(): Locale {
  return useSyncExternalStore((listener) => {
    listeners.add(listener);
    return () => { listeners.delete(listener); };
  }, () => locale, () => "en");
}

/** Translate UI copy only. Never pass user names, model output, IDs or API payloads here. */
export function translate(source: string, language: Locale, values: Record<string, string | number> = {}): string {
  const message = language === "zh-CN" ? zhCN[source] ?? source : source;
  return message.replace(/\{(\w+)\}/g, (placeholder, name: string) =>
    Object.hasOwn(values, name) ? String(values[name]) : placeholder);
}

export function t(source: string, values?: Record<string, string | number>): string {
  return translate(source, locale, values);
}

export function localeTag(): string { return locale === "zh-CN" ? "zh-CN" : "en-US"; }
