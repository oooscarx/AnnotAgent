import {expect,it} from "vitest";
import {exportDeliveryState} from "./TaskExportHistory";
it("does not confuse missing or inactive export receipts with completed downloads",()=>{
  expect(exportDeliveryState({},false)).toContain("未确认");expect(exportDeliveryState({},true)).toContain("正在导出");expect(exportDeliveryState({error:"failed"},true)).toBe("导出失败");
  expect(exportDeliveryState({result:{report:{},delivery:null} as never})).toBe("导出报告已保存");
});
