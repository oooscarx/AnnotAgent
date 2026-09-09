import {it,expect} from "vitest";
import {reviewOffset,reviewPath,sourceReviewPath} from "./reviewNavigation";
it("retains queue offsets but never accepts external or traversal return URLs",()=>{
  const url=new URL("http://localhost/projects/p/manage/runs/r?source_review=review&queue_offset=50");
  expect(sourceReviewPath("p",url)).toBe("/projects/p/manage/review/review?queue_offset=50");
  for(const offset of ["-1","1e3","3.5","Infinity","9007199254740992"]) {url.searchParams.set("queue_offset",offset);expect(reviewOffset(url)).toBe(0);}
  url.searchParams.set("source_review","https://evil.invalid");expect(sourceReviewPath("p",url)).toBeUndefined();
  expect(reviewPath("p",undefined,100)).toBe("/projects/p/manage/review?queue_offset=100");
});
