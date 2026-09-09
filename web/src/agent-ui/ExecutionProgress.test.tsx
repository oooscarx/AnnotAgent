import { it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { ExecutionProgress, executionElapsed, failureDetail, callStage } from "./ExecutionProgress";

it("keeps legacy unknown outcomes visible without fabricating timing or retry", () => {
  const html = renderToStaticMarkup(<ExecutionProgress receipts={[{id:"old",title:"目标规划",status:"in_doubt"}]} />);
  expect(html).toContain("本地执行已结束");
  expect(html).toContain("耗时：未记录");
  expect(html).toContain("刷新不会重新调用模型");
  expect(html).not.toContain("<button");
});
it("shows a current active receipt ahead of an older unknown outcome", () => {
  const html = renderToStaticMarkup(<ExecutionProgress receipts={[{id:"old",title:"旧规划",status:"in_doubt"},{id:"new",title:"新规划",status:"reserved"}]} />);
  expect(html).toContain('role="status">新规划');
  expect(html).not.toContain("当前没有此请求的活动执行");
});
it("uses persisted duration and never invents finished timing from local mount time", () => {
  expect(executionElapsed(undefined,undefined,1234)).toBe("1.2 秒");
  expect(executionElapsed("2026-09-09T00:00:00Z",undefined,undefined,Date.parse("2026-09-09T00:00:03Z"))).toBe("3 秒");
  expect(executionElapsed("2026-09-09T00:00:00Z")).toBe("未记录");
  expect(executionElapsed("invalid",undefined,-1,Date.now())).toBe("未记录");
});
it("presents typed error categories and HTTP codes without requiring raw provider text", () => {
  expect(failureDetail({category:"http_status",stage:"provider_request",http_status:502})).toBe("服务商返回错误 · 发送或等待模型 · HTTP 502");
  expect(failureDetail({category:"invalid_structured_output",stage:"structured_output"})).toContain("结构化结果不符合协议");
  expect(failureDetail(null)).toBeUndefined();
  expect(callStage("response_received")).toContain("已收到响应");
});
