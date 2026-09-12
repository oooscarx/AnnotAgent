import {readFileSync} from "node:fs";
import {expect,test} from "@playwright/test";

const titles:Record<string,string>={
  model_weights_missing:"本地模型缺少权重",
  provider_request_not_sent:"模型请求没有发出",
  provider_outcome_unknown:"远端结果未知",
  model_response_invalid_structure:"模型响应结构无法使用",
  legal_empty_detection:"这张样例没有检测到候选",
  candidate_projection_failed:"候选无法投影到原图",
  authorization_expired:"当前模型授权已过期",
  task_call_budget_exhausted:"当前任务的模型调用额度已用完",
};

test("typed result and authorization diagnostics remain passive and distinct",async({page,request},testInfo)=>{
  test.skip(!process.env.AGENT_UI_TEST_MANIFEST,"Requires the marked isolated Agent UI fixture");
  test.setTimeout(120_000);
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8"));
  const scenes=manifest.diagnostic_scenes?.scenes as Record<string,{task_url:string;workspace_url:string;expected:{state:string};diagnostic:{code:string;safe_action:{url:string;method:string};automatic_retry:boolean;preserves_existing_results:boolean}}> | undefined;
  expect(Object.keys(scenes||{}).sort()).toEqual(Object.keys(titles).sort());
  const writes:string[]=[];
  page.on("request",request=>{if(!["GET","HEAD","OPTIONS"].includes(request.method())&&request.url().includes("/api/"))writes.push(`${request.method()} ${request.url()}`);});
  for(const [code,scene] of Object.entries(scenes!)){
    const workspace=await(await request.get(scene.workspace_url)).json();
    const diagnostic=workspace.mainline.result_diagnostics.find((item:{code:string})=>item.code===code);
    expect(diagnostic).toMatchObject({code,state:scene.expected.state,automatic_retry:false,preserves_existing_results:true,safe_action:{method:"GET",url:scene.diagnostic.safe_action.url}});
    await page.goto(scene.task_url);
    const status=page.getByRole("region",{name:"当前任务状态",exact:true});
    await expect(status.getByText(titles[code],{exact:true})).toBeVisible();
    const safe=status.locator(`a[href="${scene.diagnostic.safe_action.url}"]`);
    await expect(safe).toBeVisible();
    await expect(safe).toHaveAttribute("target","_blank");
    await expect(status.getByRole("button",{name:"继续任务",exact:true})).toHaveCount(0);
    await expect(status.getByRole("button",{name:"确认当前范围",exact:true})).toHaveCount(0);
    await page.screenshot({path:testInfo.outputPath(`${code}.png`),fullPage:true,animations:"disabled"});
  }
  expect(writes).toEqual([]);
});
