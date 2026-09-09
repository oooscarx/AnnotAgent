import {useEffect,useRef} from "react";
import {decodeCocoRleMask,type ArtifactMask} from "../pipelinePresentation";

/** Display only masks in the original image coordinate frame; never stretch a mismatched mask. */
export function maskOverlayPixels(masks:ArtifactMask[],width:number,height:number):Uint8ClampedArray|undefined {
  if(!Number.isSafeInteger(width)||!Number.isSafeInteger(height)||width<=0||height<=0||width*height>16_000_000)return undefined;
  const pixels=new Uint8ClampedArray(width*height*4);
  for(const mask of masks){
    if(mask.width!==width||mask.height!==height)continue;
    const decoded=decodeCocoRleMask(width,height,mask.counts);if(!decoded)continue;
    decoded.forEach((active,index)=>{if(active){const offset=index*4;pixels[offset]=24;pixels[offset+1]=153;pixels[offset+2]=171;pixels[offset+3]=Math.min(150,pixels[offset+3]+82);}});
  }
  return pixels;
}
export function ArtifactMaskLayer({masks,width=masks[0]?.width,height=masks[0]?.height}:{masks:ArtifactMask[];width?:number;height?:number}) {
  const canvasRef=useRef<HTMLCanvasElement>(null);
  useEffect(()=>{
    const canvas=canvasRef.current;if(!canvas||!width||!height)return;
    const pixels=maskOverlayPixels(masks,width,height);if(!pixels)return;
    canvas.width=width;canvas.height=height;const context=canvas.getContext("2d");if(!context)return;
    const data=context.createImageData(width,height);data.data.set(pixels);context.putImageData(data,0,0);
  },[masks,width,height]);
  return width&&height?<canvas ref={canvasRef} className="artifact-mask-layer" aria-hidden="true"/>:null;
}
