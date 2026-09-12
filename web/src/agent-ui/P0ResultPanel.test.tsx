import {expect,it} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {P0ResultPanel,type P0ResultPanelView} from "./P0ResultPanel";

const service={} as never;

it("shows compact preparation state without mounting an empty review form",()=>{
  const view:P0ResultPanelView={kind:"preparing",stage:"正在生成标注方法",message:"Builder 完成后由服务端继续运行三张样例。",elapsed_ms:4100};
  const html=renderToStaticMarkup(<P0ResultPanel service={service} projectId="project" taskId="task" view={view}/>);
  expect(html).toContain("正在生成标注方法");
  expect(html).toContain("已用时 4 秒");
  expect(html).not.toContain("检查当前任务结果");
  expect(html).not.toContain("准备 Schema");
});

it("keeps a legal empty result distinct from human negative confirmation",()=>{
  const view:P0ResultPanelView={kind:"diagnostic",category:"legal_empty",message:"模型完成，但没有返回候选。",image:null,annotations:[],focus_candidate_id:null};
  const html=renderToStaticMarkup(<P0ResultPanel service={service} projectId="project" taskId="task" view={view}/>);
  expect(html).toContain("本次没有检测到候选");
  expect(html).toContain("不是人工负样本确认");
  expect(html).not.toContain("确认整张图没有目标");
});

it("moves a non-terminal coarse outcome to read-only projection diagnostics",()=>{
  const coarse={id:"coarse",image_id:"image",task_id:"objects",label:"cup",value:{kind:"bounding_box" as const,rect:[0,0,1,1] as [number,number,number,number]},attributes:{},source:"model",review_status:"needs_review" as const,provenance:{},created_at:"TEST"};
  const view:P0ResultPanelView={kind:"sample_feedback",images:[],labels:[],sample_result:{project_id:"project",conversation_id:"conversation",task_id:"task",project_schema_revision:"schema",draft_id:"draft",draft_revision:2,sample_test_id:"sample",images:[{image_id:"image",image_sha256:"pixels",result_revision:"result",candidates:[],annotations:[coarse]}]},focus:null,actions:[]};
  const html=renderToStaticMarkup(<P0ResultPanel service={service} projectId="project" taskId="task" view={view}/>);
  expect(html).toContain("候选无法投影到原图");
  expect(html).toContain("中间粗框不会进入样例审核");
  expect(html).not.toContain("样例反馈");
});

it("never renders a Sample result owned by another task",()=>{
  const candidate={id:"candidate",image_id:"image",task_id:"objects",label:"cup",value:{kind:"bounding_box" as const,rect:[0.1,0.1,0.2,0.2] as [number,number,number,number]},attributes:{},source:"model",review_status:"needs_review" as const,provenance:{},created_at:"TEST"};
  const view:P0ResultPanelView={kind:"sample_feedback",images:[{id:"image",name:"foreign"}],labels:[],sample_result:{project_id:"other-project",conversation_id:"conversation",task_id:"other-task",project_schema_revision:"schema",draft_id:"draft",draft_revision:2,sample_test_id:"sample",images:[{image_id:"image",image_sha256:"pixels",result_revision:"result",candidates:[{candidate_id:"candidate",selection:null}],annotations:[candidate]}]},focus:null,actions:[]};
  const html=renderToStaticMarkup(<P0ResultPanel service={service} projectId="project" taskId="task" view={view}/>);
  expect(html).toContain("不属于当前 Project/Task");
  expect(html).not.toContain("检查样例结果");
  expect(html).not.toContain("foreign");
});
