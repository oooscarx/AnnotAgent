import {agentPath} from "./navigationContract";
export function reviewOffset(url:URL):number {
  const raw=url.searchParams.get("queue_offset")||"0";const n=Number(raw);
  return /^\d+$/.test(raw)&&Number.isSafeInteger(n)&&n>=0&&n<=1_000_000?n:0;
}
export function reviewPath(projectId:string,reviewId?:string,offset=0):string {
  const path=agentPath(reviewId?{kind:"detail",projectId,page:"review",objectId:reviewId}:{kind:"management",projectId,page:"review"});
  return `${path}?queue_offset=${offset}`;
}
export function sourceReviewPath(projectId:string,url:URL):string|undefined {
  const id=url.searchParams.get("source_review");if(!id)return undefined;
  try{return reviewPath(projectId,id,reviewOffset(url));}catch{return undefined;}
}
