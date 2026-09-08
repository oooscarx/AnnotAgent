import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ConversationImages } from "./ConversationImages";
import type { ImageItem } from "../types";

describe("conversation browsing previews", () => {
  const original: ImageItem = {
    image_id: "image", project_id: "project", index: 0, display_index: 1,
    name: "TEST image", path: "image.png", path_snapshot: "image.png",
    content_hash: "TEST", size_bytes: 1000, status: "imported", url: "/original",
  };
  it("uses a bounded preview without changing the image's original URL", () => {
    const image = { ...original, thumbnail_url: "/thumbnail" };
    const html = renderToStaticMarkup(<ConversationImages images={[image]} onSelect={() => {}} />);
    expect(html).toContain('src="/thumbnail"');
    expect(html).not.toContain('src="/original"');
    expect(image.url).toBe("/original");
  });
  it("supports an older index without a preview field", () => {
    expect(renderToStaticMarkup(<ConversationImages images={[original]} onSelect={() => {}} />)).toContain('src="/original"');
  });
});
