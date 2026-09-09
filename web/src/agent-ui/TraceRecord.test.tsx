import {expect,it} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {TraceRecord} from "./TraceRecord";
it("shows persisted rationale and tool results without claiming unknown success",()=>{
  const html=renderToStaticMarkup(<TraceRecord index={0} value={{status:"in_doubt",evidence:{decision:{Ok:{rationale:"Inspect candidate boundaries",labels:["ball"]}}},session:{steps:[{tool_name:"inspect_crop",arguments:{image:"TEST"},result:{uncertain:true}}]}}}/>);
  expect(html).toContain("Inspect candidate boundaries");expect(html).toContain("inspect_crop");expect(html).toContain("状态未记录");expect(html).toContain("in_doubt");expect(html).not.toContain("已完成");
});
it("renders imported messages as escaped historical text, not HTML",()=>{const html=renderToStaticMarkup(<TraceRecord index={0} value={{input:{text:"<script>bad()</script>"}}}/>);expect(html).not.toContain("<script>");expect(html).toContain("&lt;script&gt;");});
