import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";

test("canonical SampleCandidate reaches the real Composer and survives as an owned message",async({page,request})=>{
  test.skip(!process.env.AGENT_UI_TEST_MANIFEST,"Requires the marked isolated Agent UI fixture");
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8"));
  expect(manifest.fixture).toBe("external-model-only");
  const scene=manifest.bbox;
  const intent=await (await request.get(`${scene.task_root}/delivery-intent`)).json();
  if(!intent.saved){
    const session=await (await request.get("/api/session")).json();
    const response=await request.post(`${scene.task_root}/delivery-intent`,{headers:{"x-annotagent-csrf":session.csrf_token},data:{
      command_id:crypto.randomUUID(),expected_revision:0,image_ids:[scene.image_id],
      label_spec:[{stable_id:"cup",display_name:"杯子",aliases:["cup"],include:"可见杯子",exclude:"瓶子"}],
      training_target:{annotation_kind:"bounding_box",framework:"ultralytics",export_profile:"ultralytics_yolo_detection",profile_revision:1},
      split_policy:{train_percent:80,seed:7,preserve_existing:true,keep_known_groups_together:true},
    }});
    expect(response.ok(),await response.text()).toBe(true);
  }
  const canonical=await (await request.get(`${scene.task_root}/visual-selections?cursor=0&limit=20`)).json();
  const selected=canonical.items.find((item:{sample_test_id:string})=>item.sample_test_id===scene.sample_test_id);
  const image=selected.images.find((item:{image_id:string})=>item.image_id===scene.image_id);
  const candidate=image.candidates.find((item:{candidate_id:string})=>item.candidate_id===scene.candidate_id);
  expect(candidate.source_artifact_id).toBe(scene.source_artifact_id);

  const writes:string[]=[];
  page.on("request",event=>{if(!["GET","HEAD"].includes(event.method()))writes.push(`${event.method()} ${new URL(event.url()).pathname}`);});
  await page.goto(`/projects/${scene.project}/work?task=${scene.task_id}&delivery_view=sample&delivery_image=${scene.image_id}`);
  const review=page.getByRole("region",{name:"当前任务图片结果",exact:true});
  await expect(review.locator("rect.aa-annotation-shape")).toHaveCount(1);
  await review.locator("rect.aa-annotation-shape").click();
  await review.getByRole("button",{name:"这个样例框有问题",exact:true}).click();
  await expect(page.locator(".reference-chip")).toContainText(scene.candidate_id);
  await page.getByRole("textbox",{name:"给 AnnotAgent 的需求"}).fill("TEST candidate-specific boundary feedback");
  const sent=page.waitForResponse(event=>event.request().method()==="POST"&&new URL(event.url()).pathname.endsWith("/send"));
  await page.getByRole("button",{name:"发送",exact:true}).dblclick();
  const response=await sent;
  expect(response.ok(),await response.text()).toBe(true);
  const body=response.request().postDataJSON();
  expect(body.message.image).toEqual({image_id:scene.image_id,sha256:scene.content_hash});
  expect(body.message.reference).toEqual({scope:"sample_candidate",task_id:scene.task_id,project_schema_revision:selected.project_schema_revision,draft_id:scene.draft_id,draft_revision:selected.draft_revision,sample_test_id:scene.sample_test_id,candidate_id:scene.candidate_id,source_artifact_id:scene.source_artifact_id});
  expect(writes.filter(item=>item.endsWith("/send"))).toHaveLength(1);
  const thread=await (await request.get(`${scene.task_root}/thread?limit=100`)).json();
  const persisted=thread.items.find((item:{message:{input:{id:string}}})=>item.message.input.id===body.message.id);
  expect(persisted?.message.input.reference).toEqual(body.message.reference);
  const before=writes.length;
  await page.reload();
  await expect(page.locator(".user-message").filter({hasText:"TEST candidate-specific boundary feedback"}).last()).toBeVisible();
  expect(writes).toHaveLength(before);
});

test("formal review and package readiness are passive on mount and restore the task lineage",async({page,request})=>{
  test.skip(!process.env.AGENT_UI_TEST_MANIFEST,"Requires the marked isolated Agent UI fixture");
  const manifest=JSON.parse(readFileSync(process.env.AGENT_UI_TEST_MANIFEST!,"utf8"));
  const scene=manifest.bbox;
  const writes:string[]=[];
  page.on("request",event=>{if(!["GET","HEAD"].includes(event.method()))writes.push(`${event.method()} ${new URL(event.url()).pathname}`);});
  await page.goto(`/projects/${scene.project}/work?task=${scene.task_id}&delivery_view=formal&delivery_image=${scene.image_id}`);
  await expect(page.getByRole("region",{name:"当前任务图片结果",exact:true})).toContainText("当前任务还没有绑定正式处理结果");
  await expect(page.getByRole("region",{name:"训练数据包交付",exact:true})).toContainText(/尚未满足打包条件|正式审核齐全/);
  expect(writes).toEqual([]);
  await page.reload();
  expect(writes).toEqual([]);
});
