import { test, expect } from "@playwright/test";
test("reference SVG gallery has named controls and identical 16/18 sizes in both themes",async({page,request})=>{
  expect((await request.get("/brand/core/ui-icons.svg")).ok()).toBe(true);
  await page.goto("/ui-preview?icons=1");
  await expect(page.locator(".icon-gallery-grid section")).toHaveCount(29);
  for(const dark of [false,true]){
    if(dark)await page.getByRole("button",{name:"切换浅色 / 深色"}).click();
    for(const size of [16,18])expect(await page.locator(`.icon-gallery-grid > section:first-child > div > svg[width="${size}"]`).evaluate(el=>el.getBoundingClientRect().width)).toBe(size);
    const button=page.getByRole("button",{name:"plus active",exact:true});await button.focus();await expect(button).toBeFocused();await button.hover();
    await expect(page.getByRole("button",{name:"plus disabled",exact:true})).toBeDisabled();
  }
});
for(const width of [1440,1280,1024,390])test(`visual finish chrome and viewport containment ${width}`,async({page})=>{
  await page.setViewportSize({width,height:width===390?844:720});
  await page.goto("/ui-preview?task=new");
  const failures:string[]=[];page.on("response",r=>{if(r.status()>=400)failures.push(r.url());});
  await expect(page.getByRole("button",{name:"发送",exact:true})).toBeVisible();
  if(width>760){await expect(page.locator(".mobile-menu")).toBeHidden();expect((await page.locator(".project-sidebar").boundingBox())!.width).toBe(216);}
  await page.getByLabel("项目管理菜单").click();
  await expect(page.locator(".project-menu")).toHaveAttribute("style",/left:/);
  const menu=await page.locator(".project-menu").boundingBox();expect(menu!.x).toBeGreaterThanOrEqual(0);expect(menu!.x+menu!.width).toBeLessThanOrEqual(width);
  await page.keyboard.press("Escape");await expect(page.getByLabel("项目管理菜单")).toBeFocused();
  const picker=page.getByRole("button",{name:/选择模型：/});await picker.click();
  const box=await page.locator(".model-picker").boundingBox();expect(box!.x).toBeGreaterThanOrEqual(0);expect(box!.x+box!.width).toBeLessThanOrEqual(width);expect(box!.y).toBeGreaterThanOrEqual(0);
  await page.keyboard.press("Escape");await expect(picker).toBeFocused();
  expect(failures).toEqual([]);
});
test("canvas label and handle scale stays in screen units without changing geometry",async({page})=>{
  await page.goto("/ui-preview?task=scene&pane=image");
  const rect=page.locator(".canvas-region svg > g > rect").first();await expect(rect).toBeVisible();
  const geometry=await rect.evaluate(el=>["x","y","width","height"].map(k=>el.getAttribute(k)));
  const label=page.locator(".bbox-label text").first();
  const size=()=>label.evaluate(el=>{const t=el as SVGTextElement;return parseFloat(getComputedStyle(t).fontSize)*t.getScreenCTM()!.a;});
  expect(await size()).toBeCloseTo(12,1);
  await page.getByLabel("图片缩放").fill("200");
  await expect.poll(size).toBeCloseTo(12,1);
  expect(await rect.evaluate(el=>["x","y","width","height"].map(k=>el.getAttribute(k)))).toEqual(geometry);
  expect(await rect.getAttribute("stroke")).toBe(await label.getAttribute("fill"));
});
