import {expect,it} from "vitest";
import {editableHumanKinds,humanAnnotationValue,newHumanAnnotation,relabelHumanAnnotation} from "./humanGeometry";
import type {ProjectSummary} from "../types";
const task=(kind:string,labels=["cup","bottle"])=>({id:"t",display_name:"TEST",kind,labels,required:false}) as ProjectSummary["annotation_schema"][number];
it("preserves all former manual annotation types without invented model confidence",()=>{
  for(const kind of ["classification","bounding_box","polygon","semantic_mask","instance_mask","keypoints","polyline"]){
    expect(editableHumanKinds.has(kind)).toBe(true);
    const a=newHumanAnnotation("image",task(kind),"id","now");
    expect(a.value.kind).toBe(kind);expect(a.source).toBe("human");expect(a.confidence).toBeUndefined();expect(a.review_status).toBe("needs_review");
  }
  expect(()=>newHumanAnnotation("i",task("relation"),"a","now")).toThrow();
  expect(()=>newHumanAnnotation("i",task("bounding_box",[]),"a","now")).toThrow();
  expect(humanAnnotationValue("semantic_mask","cup")).toMatchObject({kind:"semantic_mask",mask:{encoding:"polygon"}});
});
it("updates the classification payload with its visible label and preserves geometry otherwise",()=>{
  const a=newHumanAnnotation("i",task("classification"),"a","now");
  expect(relabelHumanAnnotation(a,"bottle")).toMatchObject({id:"a",label:"bottle",value:{kind:"classification",labels:["bottle"]}});
  expect(a.value).toEqual({kind:"classification",labels:["cup"]});
  const box=newHumanAnnotation("i",task("bounding_box"),"b","now");expect(relabelHumanAnnotation(box,"bottle").value).toEqual(box.value);
});
