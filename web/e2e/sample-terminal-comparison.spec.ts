import {test,expect as baseExpect} from "./fixtures";
import {sample} from "./conversation-feedback-helpers";
import {isolatedEvidencePath} from "./evidence";

const expect=baseExpect.configure({timeout:75_000});

test("TEST projection transport: comparison excludes coarse aggregation and preserves empty finals",async({page,request})=>{
  test.setTimeout(180_000);
  const state=await sample(request,page,"terminal-comparison",true);
  const projection=state.record.report.samples[0].projection;
  const expected=new Set([...projection.final_candidates,...projection.review_candidates.map((item:any)=>item.candidate)].map((item:any)=>item.outcome.id)).size;
  expect(expected).toBeGreaterThan(0);
  // Browser-only response perturbation, not a saved model result or a quality
  // comparison. The underlying Project, image, sample and inference are real
  // isolated TEST service records. No user workspace is involved.
  let empty=false;
  await page.route(url=>url.pathname===`/api/workflow-drafts/${state.record.draft_id}/sample-test`,async route=>{
    const response=await route.fetch();expect(response.ok()).toBe(true);
    const body=await response.json();
    expect(body.sample_test.id).toBe(state.record.id);
    const result=body.sample_test.report.samples[0];
    result.outcomes.push({...state.candidate.outcome,id:"TEST-intermediate-must-not-render",label:"TEST coarse aggregation",value:{kind:"bounding_box",rect:[0,0,1,1]}});
    if(empty){result.projection.final_candidates=[];result.projection.review_candidates=[];result.projection.no_target=true;}
    await route.fulfill({response,json:body});
  });
  await page.route(url=>url.pathname===`/api/projects/${state.project}/sample-plan-copies/${state.record.draft_id}`,route=>route.fulfill({json:{project_id:state.project,sample_test_id:state.record.id,baseline_draft_id:state.record.draft_id,feedback:[]}}));
  const writes:string[]=[];
  page.on("request",req=>{if(!["GET","HEAD","OPTIONS"].includes(req.method())&&new URL(req.url()).pathname.startsWith("/api/"))writes.push(req.url());});
  await page.goto(state.url);
  const canvas=page.getByRole("region",{name:"Saved sample results",exact:true});
  await expect(canvas.locator("svg.annotation-canvas")).toHaveAttribute("aria-label",`${expected} annotations over the active image`);
  await canvas.getByRole("button",{name:"Before adjustment",exact:true}).click();
  await expect(canvas.locator("svg.annotation-canvas")).toHaveAttribute("aria-label",`${expected} annotations over the active image`);
  await expect(canvas).not.toContainText("TEST coarse aggregation");
  await page.screenshot({path:isolatedEvidencePath("../docs/execution/conversational-workspace/terminal-only-comparison.png"),fullPage:true});
  empty=true;
  await page.reload();
  await canvas.getByRole("button",{name:"Before adjustment",exact:true}).click();
  await expect(canvas.locator("svg.annotation-canvas")).toHaveAttribute("aria-label","0 annotations over the active image");
  await canvas.getByRole("button",{name:"Current candidates",exact:true}).click();
  await expect(canvas.locator("svg.annotation-canvas")).toHaveAttribute("aria-label","0 annotations over the active image");
  expect(writes).toEqual([]);
});
