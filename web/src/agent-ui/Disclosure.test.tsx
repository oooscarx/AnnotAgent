import { expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { Disclosure } from "./Disclosure";
import sprite from "../../public/brand/core/ui-icons.svg?raw";
it("keeps native keyboard semantics and a single decorative SVG toggle", () => {
  const html = renderToStaticMarkup(<Disclosure title="计划详情" open><p>模型来源</p></Disclosure>);
  expect(html).toContain("<details");
  expect(html).toContain("open");
  expect(html.match(/<summary>/g)).toHaveLength(1);
  expect(html.match(/<svg /g)).toHaveLength(1);
  expect(html).not.toContain("<button");
  expect(html).toContain('aria-hidden="true"');
  expect(html).toContain('width="14"');
  expect(sprite).toContain("M9 7 L13.3 11.3 Q14 12 13.3 12.7 L9 17");
});
