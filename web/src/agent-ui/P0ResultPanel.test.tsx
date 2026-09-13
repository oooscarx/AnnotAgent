import {expect,it} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {P0ResultPanel,type P0ResultPanelView} from "./P0ResultPanel";

const service={} as never;
Object.defineProperty(globalThis,"location",{value:new URL("http://127.0.0.1/projects/project/work"),configurable:true});

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

it.each([
  ["legal_empty_detection","没有检测到候选；这不是人工确认的负样本。"],
  ["candidate_projection_failed","候选无法安全投影到原图；其他有效候选仍保留。"],
] as const)("keeps valid candidates visible beside mixed diagnostic %s",(code,copy)=>{
  const candidate={id:"valid-candidate",image_id:"valid-image",task_id:"objects",label:"cup",value:{kind:"bounding_box" as const,rect:[0.1,0.1,0.2,0.2] as [number,number,number,number]},attributes:{},source:"model",review_status:"needs_review" as const,provenance:{},created_at:"TEST"};
  const view:P0ResultPanelView={
    kind:"sample_feedback",images:[{id:"empty-image",name:"empty.png"},{id:"valid-image",name:"valid.png"}],labels:[{stable_id:"cup",display_name:"杯子"}],focus:null,actions:[],
    sample_result:{project_id:"project",conversation_id:"conversation",task_id:"task",project_schema_revision:"schema",draft_id:"draft",draft_revision:2,sample_test_id:"sample",images:[
      {image_id:"empty-image",image_sha256:"empty",result_revision:"empty-result",candidates:[],annotations:[]},
      {image_id:"valid-image",image_sha256:"valid",result_revision:"valid-result",candidates:[{candidate_id:candidate.id,selection:null}],annotations:[candidate]},
    ]},
    diagnostics:[{code,category:"result",state:code==="legal_empty_detection"?"completed":"blocked",source:{kind:"sample_test",id:"sample",image_index:0},automatic_retry:false,preserves_existing_results:true,safe_action:{id:"inspect",method:"GET",url:"/api/TEST/sample"},...(code==="legal_empty_detection"?{human_negative_recorded:false}:{})}],
  };
  const html=renderToStaticMarkup(<P0ResultPanel service={service} projectId="project" taskId="task" view={view}/>);
  expect(html).toContain("检查样例结果 · 2 张");
  expect(html).toContain(copy);
  expect(html).toContain("valid.png");
  expect(html).not.toContain("确认整张图没有目标");
});

it("renders fixture-backed terminal lineage evidence instead of inferring SAM execution from the Draft",()=>{
  const candidate={id:"ball",image_id:"image",task_id:"objects",label:"ball",value:{kind:"bounding_box" as const,rect:[0.4,0.4,0.1,0.1] as [number,number,number,number]},attributes:{},source:"model",review_status:"needs_review" as const,provenance:{},created_at:"TEST"};
  const view:P0ResultPanelView={kind:"sample_feedback",images:[{id:"image",name:"ball.png"}],labels:[{stable_id:"ball",display_name:"足球"}],focus:null,actions:[],sample_result:{
    project_id:"project",conversation_id:"conversation",task_id:"task",project_schema_revision:"schema",draft_id:"draft",draft_revision:2,sample_test_id:"sample",
    images:[{image_id:"image",image_sha256:"pixels",result_revision:"result",candidates:[{candidate_id:"ball",selection:null}],annotations:[candidate],execution_evidence:{configured_refiner:true,candidates:[{candidate_id:"ball",lineage_id:"detection:ball",source:"vlm_only",executed:false,reason:"方案包含精修，但本图实际 lineage 停在 Coverage Gate：PartiallyCovered。",items:[{kind:"gate",label:"Coverage Gate",node_id:"coverage",status:null,artifact_id:"coverage-id",artifact_ref:"coverage:set",detail:"PartiallyCovered"}]}]}}],
  }};
  const html=renderToStaticMarkup(<P0ResultPanel service={service} projectId="project" taskId="task" view={view}/>);
  expect(html).toContain("本图终端候选来源：VLM-only");
  expect(html).toContain("方案包含但本图未执行提示分割");
  expect(html).toContain("PartiallyCovered");
  expect(html).not.toContain("提示分割已执行");
});
