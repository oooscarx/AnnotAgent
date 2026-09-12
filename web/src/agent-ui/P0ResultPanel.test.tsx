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
