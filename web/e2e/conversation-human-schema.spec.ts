import { isolatedEvidencePath } from "./evidence";
import {expect,test} from "./fixtures";

for(const kind of ["bounding_box","classification"]){
  test(`human ${kind} labels save and restore without a planner or inference`,async({page,request})=>{
    const project=`human-schema-${kind}-${Date.now()}`;
    const created=await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST human labels without LLM\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}});
    expect(created.ok(),await created.text()).toBe(true);
    let modelRequests=0;
    page.on("request",req=>{if(/schema-proposals|schema-preview|builder-proposals|sample-operations/.test(req.url()))modelRequests++;});
    await page.goto(`/projects/${project}/work`);
    await page.getByLabel("Your message",{exact:true}).fill("TEST 自己定义标注目标，不调用模型");
    await page.getByRole("button",{name:"Save message",exact:true}).click();
    await page.getByRole("button",{name:"Define labels myself · no LLM needed",exact:true}).click();
    const form=page.getByRole("region",{name:"Define labels without a model",exact:true});
    await form.getByLabel("Output type",{exact:true}).selectOption(kind);
    const labels=kind==="bounding_box"?"黄色物块\n机器人":"室内\n室外";
    await form.getByLabel("Labels · one per line",{exact:true}).fill(labels);
    await form.getByLabel("Boundary rules · optional",{exact:true}).fill("TEST 不包含背景");
    await page.setViewportSize({width:390,height:844});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
    await page.screenshot({path:isolatedEvidencePath(`../docs/execution/conversational-workspace/human-schema-${kind}-390.png`),fullPage:true,animations:"disabled"});
    await page.setViewportSize({width:1280,height:800});
    let intercepted=false;
    await page.route("**/human-schema-drafts",async route=>{
      if(route.request().method()==="POST"&&!intercepted){intercepted=true;await route.fetch();await route.abort("failed");}else await route.continue();
    });
    await form.getByRole("button",{name:"Save label draft without a model",exact:true}).click();
    await expect(form.getByRole("button",{name:"Retry same label save",exact:true})).toBeVisible();
    await expect(form.getByLabel("Labels · one per line",{exact:true})).toHaveValue(labels);
    await form.getByRole("button",{name:"Retry same label save",exact:true}).click();
    const editor=page.getByRole("region",{name:"Saved label draft",exact:true});
    await expect(editor).toContainText("Revision 1");
    await expect(page.getByText("Human-defined labels · No model call was used to create this Schema Draft.",{exact:true})).toBeVisible();
    await page.reload();
    await expect(editor).toContainText("Revision 1");
    await editor.getByRole("button",{name:"Edit labels and boundary rules",exact:true}).click();
    await editor.getByLabel("Labels · one per line",{exact:true}).fill(`${labels}\n其他`);
    await editor.getByRole("button",{name:"Save Schema changes",exact:true}).click();
    await expect(editor).toContainText("Revision 2");
    await page.reload();
    await expect(editor).toContainText("其他");
    await editor.getByRole("button",{name:"Edit labels and boundary rules",exact:true}).scrollIntoViewIfNeeded();
    await expect(editor.getByRole("button",{name:"Edit labels and boundary rules",exact:true})).toBeInViewport();
    expect(await editor.getByRole("button",{name:"Edit labels and boundary rules",exact:true}).evaluate(element=>{
      const box=element.getBoundingClientRect();return element.contains(document.elementFromPoint(box.x+box.width/2,box.y+box.height/2));
    })).toBe(true);
    await page.screenshot({path:isolatedEvidencePath(`../docs/execution/conversational-workspace/human-schema-${kind}.png`),fullPage:true,animations:"disabled"});
    const conversation=(await (await request.get(`/api/projects/${project}/conversations`)).json()).conversation_id;
    const taskRoot=`/api/projects/${project}/conversations/${conversation}/tasks`;
    const tasks=await (await request.get(taskRoot)).json();expect(tasks).toHaveLength(1);
    const root=`${taskRoot}/${tasks[0].input.id}`;
    const schemas=await (await request.get(`${root}/human-schema-drafts`)).json();expect(schemas).toHaveLength(1);
    expect(schemas[0].revision).toBe(2);expect(schemas[0].source_call_id).toBeNull();expect(schemas[0].source_request_id).toBeTruthy();
    expect(schemas[0].definition.task.kind).toBe(kind);
    const budget=await (await request.get(`${root}/budget`)).json();
    expect(budget.total_reserved_calls).toBe(0);expect(budget.total_authorized_calls).toBe(0);
    expect(await (await request.get(`${root}/calls`)).json()).toHaveLength(0);
    expect((await request.get(`${root.replace(conversation,crypto.randomUUID())}/human-schema-drafts`)).ok()).toBe(false);
    expect((await request.post(`${root}/human-schema-drafts`,{data:{request_id:crypto.randomUUID(),decision:{decision:"clarify",question:"TEST",rationale:"TEST"},authorize:true}})).ok()).toBe(false);
    expect(modelRequests).toBe(0);
  });
}
