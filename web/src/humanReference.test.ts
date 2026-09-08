import {describe,it,expect} from "vitest";
import {humanReferenceReady} from "./humanReference";
import type {Annotation} from "./types";
const seed:Annotation={id:"human-sample:ref",image_id:"image",task_id:"sample",label:"cup",value:{kind:"bounding_box",rect:[0.4,0.4,0.2,0.2]},attributes:{},source:"human sample feedback",review_status:"needs_review",provenance:{addition_id:"ref"},created_at:""};
describe("reference target submission",()=>{
  it("requires the exact reference and a real box edit, not a seed or label-only change",()=>{
    expect(humanReferenceReady(seed,"ref",seed)).toBe(false);
    expect(humanReferenceReady({...seed,label:"bottle"},"ref",seed)).toBe(false);
    const edited:Annotation={...seed,value:{kind:"bounding_box",rect:[0.1,0.2,0.3,0.4]}};
    expect(humanReferenceReady(edited,"ref",seed)).toBe(true);
    expect(humanReferenceReady(edited,"another",seed)).toBe(false);
    expect(humanReferenceReady(edited,"ref",undefined)).toBe(false);
  });
  it("requires an explicit category change and a nonempty label",()=>{
    const initial:Annotation={...seed,label:"",value:{kind:"classification",labels:[]}};
    expect(humanReferenceReady(initial,"ref",initial)).toBe(false);
    expect(humanReferenceReady({...initial,label:"indoor",value:{kind:"classification",labels:["indoor"]}},"ref",initial)).toBe(true);
  });
});
