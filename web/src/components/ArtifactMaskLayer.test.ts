import {it,expect} from "vitest";
import {maskOverlayPixels} from "./ArtifactMaskLayer";
it("projects column-major RLE without changing source pixels or scaling foreign dimensions",()=>{
  const mask={id:"m",width:2,height:2,counts:[1,1,2]};const pixels=maskOverlayPixels([mask],2,2)!;
  expect(Array.from(pixels.filter((_,i)=>i%4===3))).toEqual([0,0,82,0]);
  expect(mask.counts).toEqual([1,1,2]);
  expect(maskOverlayPixels([mask],3,2)!.every(v=>v===0)).toBe(true);
  expect(maskOverlayPixels([{...mask,counts:[1,9]}],2,2)!.every(v=>v===0)).toBe(true);
  expect(maskOverlayPixels([],100000,100000)).toBeUndefined();
});
