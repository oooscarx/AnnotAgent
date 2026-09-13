import { expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { CurrentTaskStatus } from "./CurrentTaskStatus";

it("renders exact planner/workflow/refiner evidence without claiming authorized-only SAM ran",()=>{
  const html=renderToStaticMarkup(<CurrentTaskStatus
    presentation={{
      kind:"ready_to_start",title:"方案已保存",detail:"继续样例",primary:{kind:"prepare_sample"},
      modelBindings:{
        planningModel:"model-profile:glm-5.2",
        operationModel:null,
        workflowModels:["Qwen VLM"],
        authorizationBindings:["model-instance:efficient-sam · digest-1234"],
        refiner:{state:"authorized_only",models:["model-instance:efficient-sam"],detail:"精修模型只在授权范围，尚无证据表明它进入 Draft。"},
      },
    }}
    busy={false}
    onPrimary={vi.fn()}
    onEditTask={vi.fn()}
  />);
  expect(html).toContain("model-profile:glm-5.2");
  expect(html).toContain("Qwen VLM");
  expect(html).toContain("仅在授权范围");
  expect(html).not.toContain("已进入 Draft");
  expect(html).not.toContain("已经实际调用");
});

it("keeps the stop control available while another UI request is busy",()=>{
  const html=renderToStaticMarkup(<CurrentTaskStatus
    presentation={{kind:"running",title:"模型请求正在执行",detail:"等待回执",primary:{kind:"stop"}}}
    busy
    onPrimary={vi.fn()}
    onEditTask={vi.fn()}
  />);
  expect(html).toContain("停止当前操作");
  expect(html).not.toContain("disabled");
});

it("labels a server-issued Sample continuation without asking for Schema again",()=>{
  const html=renderToStaticMarkup(<CurrentTaskStatus
    presentation={{
      kind:"ready_to_start",title:"方案已保存，可以继续测试样例",detail:"只执行冻结样例",primary:{kind:"prepare_sample"},
      action:{id:"test_pipeline_samples",state:"requires_confirmation",method:"GET",url:"/scope",execution_method:"POST",execution_url:"/execute",requires_confirmation:true,reason:null},
    }}
    busy={false}
    onPrimary={vi.fn()}
    onEditTask={vi.fn()}
  />);
  expect(html).toContain("继续测试已保存方案");
  expect(html).not.toContain("重新生成 Schema");
});
