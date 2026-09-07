import { describe, expect, it } from "vitest";
import { resolveLocale, translate } from "./i18n";
import { zhCN } from "./locales/zh-CN";

describe("interface localization", () => {
  it("prefers a saved choice and otherwise follows the primary browser language", () => {
    expect(resolveLocale("en", ["zh-CN", "en"])).toBe("en");
    expect(resolveLocale("zh-CN", ["en-US"])).toBe("zh-CN");
    expect(resolveLocale(null, ["zh-TW"])).toBe("zh-CN");
    expect(resolveLocale("invalid", ["de-DE", "zh-CN"])).toBe("en");
    expect(resolveLocale(null, [])).toBe("en");
  });

  it("interpolates counts without English plural suffixes or translating user values", () => {
    expect(translate("{count} items selected", "zh-CN", { count: 3 })).toBe("已选择 3 项");
    expect(translate("Select Pipeline {name}", "zh-CN", { name: "Review" })).toBe("选择流程 Review");
    expect(translate("Select Pipeline {name}", "en", { name: "$& {count}" })).toBe("Select Pipeline $& {count}");
    expect(translate("Unregistered diagnostic", "zh-CN")).toBe("Unregistered diagnostic");
  });

  it("preserves interpolation arguments across every translation", () => {
    const placeholders = (value: string) => [...value.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
    for (const [source, translation] of Object.entries(zhCN)) {
      expect(placeholders(translation), source).toEqual(placeholders(source));
      expect(translate(source, "en"), source).toBe(source);
    }
  });
});
