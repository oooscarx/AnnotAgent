import {expect,it} from "vitest";
import {probeCost} from "./ProbeUsage";
it("does not present absent, invalid or zero recorded probe costs as free",()=>{
  for(const cost of ["","0","0.000","-1","unknown","Infinity"])expect(probeCost({cost,currency:"USD"})).toContain("未核实");
  expect(probeCost({cost:"0.025",currency:"USD"})).toBe("USD 0.025（服务器估算，非账单）");
});
