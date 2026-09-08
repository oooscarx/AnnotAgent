import {randomUUID} from "node:crypto";
import {test,expect as baseExpect} from "./fixtures";
import {sample} from "./conversation-feedback-helpers";
import {isolatedEvidencePath} from "./evidence";
const expect=baseExpect.configure({timeout:75_000});

for(const bbox of [true,false])test(`reference ${bbox?"box":"category"} submits through the actual human request and restores its checkpoint`,async({page,request})=>{
  test.setTimeout(180_000);
  const state=await sample(request,page,"reference-target",bbox);
  // Resolve fixture-created review requests explicitly; never bypass the real
  // pending-request policy or cancel anything outside this isolated TEST task.
  for(const waiting of state.savedRequests){
    const response=await request.post(`${state.taskRoot}/human-requests/${waiting.input.id}/cancel`);
    expect(response.ok(),await response.text()).toBe(true);
  }
  const input={id:randomUUID(),task_id:state.task,conversation_id:state.conversation,sample_test_id:state.record.id,image_id:state.image.image_id,content_hash:state.image.content_hash,addition_id:randomUUID(),expected_feedback_sequence:0,reason_code:"identify_target",question:"TEST: provide a new reference target, not a correction to the saved prediction.",resume_checkpoint_ref:randomUUID()};
  const path=`${state.taskRoot}/human-requests`;
  const created=await request.post(path,{data:input});expect(created.ok(),await created.text()).toBe(true);
  const calls=await (await request.get(`${state.taskRoot}/calls`)).json();
  const url=`${state.url}&request=${input.id}`;
  await page.goto(url);
  const canvas=page.getByRole("region",{name:"Saved sample results",exact:true});
  const submit=canvas.getByRole("button",{name:"Submit reference target",exact:true});
  await expect(submit).toBeDisabled();
  await canvas.getByRole("button",{name:"Add reference target",exact:true}).click();
  await expect(submit).toBeDisabled();
  if(bbox)await canvas.locator(".sample-feedback-coordinates input").first().fill("0.15");
  else await canvas.getByLabel("Correct label",{exact:true}).fill("室内");
  await expect(submit).toBeEnabled();
  const answers:unknown[]=[];
  page.on("request",req=>{if(req.method()==="POST"&&req.url().endsWith(`/${input.id}/answer`))answers.push(req.postDataJSON());});
  await submit.click();
  const savedRequest=async()=> (await (await request.get(path)).json()).find((value:any)=>value.input.id===input.id);
  await expect.poll(async()=> (await savedRequest()).status).toBe("applied");
  const saved=await savedRequest();
  expect(saved.answer.outcome_id).toBeNull();expect(saved.answer.addition_id).toBe(input.addition_id);
  expect(saved.answer.reason).toBe("missing_target");expect(saved.resume_draft_id).toBe(input.resume_checkpoint_ref);
  expect(answers).toHaveLength(1);
  const replay=await request.post(`${path}/${input.id}/answer`,{data:{answer:saved.answer}});
  expect(replay.ok(),await replay.text()).toBe(true);
  await page.reload();
  await expect(canvas.getByLabel("Result to inspect",{exact:true})).toHaveValue(`human-sample:${input.addition_id}`);
  const feedbackResponse=await request.get(`/api/workflow-sample-tests/${state.record.id}/images/${state.image.image_id}/feedback`);
  expect(feedbackResponse.ok(),await feedbackResponse.text()).toBe(true);
  const feedback=await feedbackResponse.json();
  expect(feedback.revisions).toHaveLength(1);
  expect(await (await request.get(`${state.taskRoot}/calls`)).json()).toEqual(calls);
  await page.screenshot({path:isolatedEvidencePath(`../docs/execution/conversational-workspace/reference-target-${bbox?"bbox":"classification"}.png`),fullPage:true});
});
