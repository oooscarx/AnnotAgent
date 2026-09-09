import { test, expect } from "@playwright/test";
import { resolve } from "node:path";

// Actual React component with explicitly synthetic service state. This is not HTTP delivery evidence.
test("whole-image review requires an explicit formal source and preserves failed confirmation retries",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async(path)=>{
    const {React,createRoot,DeliveryReview}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const state={commands:[] as unknown[],fail:true};
    Object.assign(window,{deliveryTest:state});
    const service={
      image:async(_p:string,_t:string,id:string,run:string|null)=>({intent_revision:1,intent_sha256:"intent",snapshot:{image_id:id,source_run_id:run,sha256:"snapshot",content_sha256:"image",annotations:[]},sources:[{run_id:"formal-one",model:"TEST model",status:"completed",created_at:"TEST date"}],review:null,confirmation_current:false,accepted_objects:1,unresolved_objects:0,notice:"TEST"}),
      confirmImage:async(_p:string,_t:string,input:unknown)=>{state.commands.push(input);if(state.fail)throw new Error("TEST revision conflict; no decision saved");return {revision:1};},
    };
    createRoot(host).render(React.createElement(DeliveryReview,{service,project:"TEST",task:"TEST-task",images:[{id:"image-one",name:"TEST original",src:"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII="}]}));
  },`/@fs/${resolve("e2e/ui-preview/delivery-review-harness.tsx")}`);
  await expect(page.getByRole("heading",{name:"逐张确认训练图片"})).toBeVisible();
  const positive=page.getByRole("button",{name:"确认整张图标注完整",exact:true});
  await expect(positive).toBeDisabled();
  await expect(page.getByRole("button",{name:"确认整张图没有目标",exact:true})).toBeDisabled();
  await page.getByLabel("正式标注来源",{exact:true}).selectOption("formal-one");
  await expect(page).toHaveURL(/delivery_run=formal-one/);
  await expect(positive).toBeEnabled();
  await positive.click();
  await expect(page.getByRole("alert")).toContainText("no decision saved");
  await positive.click();
  const commands=await page.evaluate(()=>(window as unknown as {deliveryTest:{commands:unknown[]}}).deliveryTest.commands);
  expect(commands).toHaveLength(2);expect(commands[0]).toEqual(commands[1]);
  await expect(page.getByText("整图决定已保存；",{exact:false})).toHaveCount(0);
  await expect(page.getByRole("button",{name:"明确排除此图",exact:true})).toBeDisabled();
  await page.getByLabel("检查备注／排除原因").fill("TEST unresolved omission");
  await expect(page.getByRole("button",{name:"明确排除此图",exact:true})).toBeEnabled();
  await page.goBack();
  await expect(page.getByLabel("正式标注来源",{exact:true})).toHaveValue("");
  await expect(positive).toBeDisabled();
});
