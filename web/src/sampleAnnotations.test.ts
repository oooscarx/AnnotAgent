import {describe,it,expect} from "vitest";
import {sampleAnnotations} from "./sampleAnnotations";
import type {SampleTestOutcomeRecord} from "./types";
describe("saved sample projection",()=>{
  it("retains original values and omits outcomes without geometry or categories",()=>{
    const box:SampleTestOutcomeRecord={id:"box",label:"cup",status:"needs_review",confidence:0.7,value:{kind:"bounding_box",rect:[0.1,0.2,0.3,0.4]}};
    const category:SampleTestOutcomeRecord={id:"category",label:"original",status:"needs_review",value:{kind:"classification",labels:["室内","测试"]}};
    const before=JSON.stringify([box,category]);
    const result=sampleAnnotations([box,category,{id:"none",label:"unknown",status:"invalid"}],"image","test");
    expect(result).toHaveLength(2);expect(result[0].value).toEqual(box.value);
    expect(result[1].label).toBe("室内, 测试");expect(result[0].provenance.sample_test_id).toBe("test");
    expect(JSON.stringify([box,category])).toBe(before);
  });
});
