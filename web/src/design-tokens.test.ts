import { describe, expect, it } from "vitest";
import tokens from "../../design/annotagent-visual-system/tokens/tokens.json";

function luminance(hex: string) {
  const channels = hex.slice(1).match(/../g)!.map(value => {
    const channel = parseInt(value, 16) / 255;
    return channel <= .04045 ? channel / 12.92 : ((channel + .055) / 1.055) ** 2.4;
  });
  return channels[0] * .2126 + channels[1] * .7152 + channels[2] * .0722;
}
describe("canonical Paper and Graphite tokens", () => {
  for (const [name, theme] of Object.entries(tokens.themes)) {
    it(`${name} keeps ordinary text and filled controls readable`, () => {
      for (const [foreground, background] of [[theme.text, theme.bg], [theme["text-muted"], theme.surface], [theme["on-primary"], theme.primary], [theme["on-danger"], theme.danger], [theme["on-warning"], theme.warning], [theme["on-success"], theme.success]]) {
        const a = luminance(foreground), b = luminance(background);
        expect((Math.max(a, b) + .05) / (Math.min(a, b) + .05)).toBeGreaterThanOrEqual(4.5);
      }
      expect(theme["checkbox-mark"]).toContain(encodeURIComponent(theme["on-primary"]));
    });
  }
  it("does not recolor the established annotation palette with interface colors", () => {
    expect(Array.from({length: 8}, (_, i) => tokens.common[`annotation-${i + 1}` as keyof typeof tokens.common])).toEqual([
      "#2563EB", "#00A896", "#7C3AED", "#F59E0B", "#E11D48", "#16A34A", "#0EA5E9", "#F97316",
    ]);
  });
});
