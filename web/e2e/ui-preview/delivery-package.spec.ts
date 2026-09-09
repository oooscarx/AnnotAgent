import {test,expect} from "@playwright/test";
import {resolve} from "node:path";
test("package card uses owned persisted status and never packages on mount or status retry",async({page})=>{
  await page.goto("/ui-preview?task=new");
  await page.evaluate(async path=>{
    const {React,createRoot,DeliveryPackage}=await import(path);
    const host=document.createElement("main");document.body.replaceChildren(host);
    const state={writes:[] as unknown[],ready:false};Object.assign(window,{packageTest:state});
    const service={history:async()=>({items:[{id:"old-package",created_at:"TEST saved"}],next_cursor:null}),
      image:async(_p:string,_t:string,image:string,run:string|null)=>({intent_revision:2,intent_sha256:"intent",confirmation_current:!!run,review:{revision:3,input:{source_run_id:"formal"}}}),
      startPackage:async(_p:string,_t:string,input:unknown)=>{state.writes.push(input);return {job:{id:(input as {command_id:string}).command_id,phase:"exporting",result:null,intent_revision:2},active:false,dispatched:true};},
      packageStatus:async(_p:string,_t:string,id:string)=>({job:{id,phase:state.ready?"ready":"validating",intent_revision:1,result:state.ready?{images:11,objects:20,negatives:1,excluded:1,bytes:12345,sha256:"TEST frozen hash",summary:{labels:["冻结类别"],splits:{train:9,val:2},warnings:["TEST near duplicates not checked"],exclusions:{excluded:"TEST incomplete"}}}:null},active:false,interrupted:!state.ready}),
      downloadUrl:()=>"/TEST-only-no-download",cancelPackage:async()=>{throw new Error("TEST unused");}};
    createRoot(host).render(React.createElement(DeliveryPackage,{service,project:"TEST",task:"TASK",scope:{revision:2,content_sha256:"intent",image_ids:["one","two"]},onInspect:()=>{}}));
  },`/@fs/${resolve("e2e/ui-preview/delivery-review-harness.tsx")}`);
  await page.getByLabel("本任务已保存的数据包",{exact:true}).selectOption("old-package");
  await expect(page.getByText("执行已中断或远端状态未知",{exact:true})).toBeVisible();
  await expect(page.getByRole("link",{name:"下载数据集 ZIP"})).toHaveCount(0);
  await page.getByRole("button",{name:"读取打包状态",exact:true}).click();
  expect(await page.evaluate(()=>(window as unknown as {packageTest:{writes:unknown[]}}).packageTest.writes)).toEqual([]);
  await page.evaluate(()=>(window as unknown as {packageTest:{ready:boolean}}).packageTest.ready=true);
  await page.getByRole("button",{name:"读取打包状态",exact:true}).click();
  await expect(page.getByText("数据集已打包",{exact:true})).toBeVisible();
  await expect(page.getByText("类别：冻结类别",{exact:true})).toBeVisible();
  await expect(page.getByText("训练图片 9 · 验证图片 2",{exact:false})).toBeVisible();
  await expect(page.getByRole("link",{name:"下载数据集 ZIP"})).toHaveAttribute("href","/TEST-only-no-download");
  await page.getByRole("button",{name:"检查当前打包范围",exact:true}).click();
  await expect(page.getByRole("button",{name:"确认并生成训练数据包",exact:true})).toBeEnabled();
  expect(await page.evaluate(()=>(window as unknown as {packageTest:{writes:unknown[]}}).packageTest.writes)).toEqual([]);
  await page.getByRole("button",{name:"确认并生成训练数据包",exact:true}).click();
  await page.getByRole("button",{name:"确认并生成训练数据包",exact:true}).click();
  const writes=await page.evaluate(()=>(window as unknown as {packageTest:{writes:unknown[]}}).packageTest.writes);
  expect(writes).toHaveLength(2);expect(writes[0]).toEqual(writes[1]);
});
