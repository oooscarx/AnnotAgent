import { expect, it } from "vitest";
import production from "../../public/brand/core/ui-icons.svg?raw";
import preview from "../../ui-preview/public/brand/core/ui-icons.svg?raw";
import css from "./ui.css?raw";
import { renderToStaticMarkup } from "react-dom/server";
import { Icon, IconButton, iconNames, type IconName } from "./Icon";
import { labelColor, annotationColor, annotationVisual } from "../annotationVisuals";
import type { Annotation } from "../types";
it("uses only existing original sprite symbols in both production and Preview",()=>{
  expect(preview).toBe(production);
  for(const name of iconNames){expect(production).toContain(`id="aa-${name}"`);expect(renderToStaticMarkup(<Icon name={name}/>)).toContain('aria-hidden="true"');}
  expect(()=>renderToStaticMarkup(<Icon name={"invented" as IconName}/>)).toThrow("Unknown UI icon");
  expect(renderToStaticMarkup(<IconButton icon="close" label="关闭图片"/>)).toContain('aria-label="关闭图片"');
});
it("has no glyph chrome, demo label branch or brand inversion in the Agent UI",()=>{
  const sources=import.meta.glob("./*.tsx",{query:"?raw",import:"default",eager:true});
  for(const file of ["App","ArtifactPane","Settings","PlanBlock"]){const source=sources[`./${file}.tsx`];expect(source).not.toMatch(/[＋⌄›▱⚙☰⌁☷▷✓■↑×]|···/);expect(source).not.toContain('b.label === "bottle"');}
  expect(css).not.toContain("filter: invert");
});
it("label-only canvas shares the established deterministic annotation mapping",()=>{
  for(const label of ["cup","bottle","黄色物块","ball","very long label"]){const annotation={label,task_id:"objects",value:{kind:"bounding_box"}} as Annotation;expect(labelColor(label)).toBe(annotationColor(annotationVisual(annotation).slot));}
});
