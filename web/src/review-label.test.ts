import { expect, it } from "vitest";
import { reviewLabelText, withReviewLabel } from "./review-label";
import type { Annotation } from "./types";

it("edits classification value and display label together without mutating the original",()=>{
  const before={label:"室内",value:{kind:"classification",labels:["室内"]}} as Annotation;
  const after=withReviewLabel(before,"室外");
  expect(after.value).toEqual({kind:"classification",labels:["室外"]});
  expect(after.label).toBe("室外");
  expect(before.value).toEqual({kind:"classification",labels:["室内"]});
  expect(reviewLabelText(withReviewLabel(before,"室内,"))).toBe("室内, ");
  expect(reviewLabelText(withReviewLabel(before,"室内, 室外"))).toBe("室内, 室外");
});
