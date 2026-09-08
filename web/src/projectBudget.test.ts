import {describe,it,expect} from "vitest";
import {projectBudgetAvailability} from "./projectBudget";
describe("Project budget snapshot",()=>{
  it("does not present missing or malformed accounting as free calls",()=>{
    expect(projectBudgetAvailability().blocked).toBe(true);
    expect(projectBudgetAvailability({revision:1,maximum_calls:3,reserved_calls:NaN}).known).toBe(false);
    expect(projectBudgetAvailability({revision:1,maximum_calls:-1,reserved_calls:0}).known).toBe(false);
  });
  it("distinguishes no ceiling from exhausted or remaining explicit allowance",()=>{
    expect(projectBudgetAvailability({revision:0,maximum_calls:null,reserved_calls:9})).toEqual({known:true,blocked:false,remaining:null});
    expect(projectBudgetAvailability({revision:1,maximum_calls:9,reserved_calls:9})).toEqual({known:true,blocked:true,remaining:0});
    expect(projectBudgetAvailability({revision:1,maximum_calls:10,reserved_calls:9})).toEqual({known:true,blocked:false,remaining:1});
  });
});
