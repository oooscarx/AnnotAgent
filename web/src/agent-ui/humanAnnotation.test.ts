import {expect,it} from "vitest";
import {newHumanAnnotation} from "./humanGeometry";
it("creates only explicit human editable geometry without model evidence or confidence",()=>{
  for(const kind of ["bounding_box","polygon","polyline","keypoints"]){
    const a=newHumanAnnotation("image",{id:"target",display_name:"Target",kind,labels:["ball"],required:false},"fixed","now");
    expect(a).toMatchObject({id:"fixed",image_id:"image",task_id:"target",source:"human",review_status:"needs_review",value:{kind},attributes:{},provenance:{artifact_ids:[]}});
    expect(a.confidence).toBeUndefined();
  }
  for(const kind of ["classification","semantic_mask","unsupported"]){expect(()=>newHumanAnnotation("i",{id:"t",display_name:"T",kind,labels:["a"],required:false},"id","now")).toThrow();}
});
