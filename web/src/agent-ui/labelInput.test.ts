import {expect,it} from "vitest";
import {splitGoalLabels} from "./labelInput";
import {parseAgentRoute} from "./navigationContract";
it("keeps explicit categories and order without model interpretation",()=>{
  expect(splitGoalLabels("杯子，盘子\nbottle, cup, cup")).toEqual(["杯子","盘子","bottle","cup"]);
  expect(splitGoalLabels(" , \n")).toEqual([]);
});
it("does not revive old Journey pages from saved scene or return queries",()=>{
  for(const scene of ["goal","images","model","samples","confirm","run","revision"]){expect(parseAgentRoute(new URL(`/projects/p/task/${scene}?draft=d&test=t&session=s&return_to=https://example.invalid`,"http://localhost"))).toEqual({kind:"not-found"});}
});
