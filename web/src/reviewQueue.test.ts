import {describe,it,expect} from "vitest";
import {canRefreshReviewDraft,mergeReviewQueue} from "./reviewQueue";
describe("review queue/detail response ordering",()=>{
  it("keeps a later exact detail while refreshing other queue entries",()=>{
    const detail={id:"selected",shape:"polygon"};
    expect(mergeReviewQueue([detail,{id:"other",shape:"old"}],[{id:"selected",shape:"classification"},{id:"other",shape:"new"}],{newerDetailId:"selected"})).toEqual([detail,{id:"other",shape:"new"}]);
  });
  it("allows a fresh explicit refresh to update a settled detail",()=>{
    expect(mergeReviewQueue([{id:"selected",status:"pending"}],[{id:"selected",status:"accepted"}],{})).toEqual([{id:"selected",status:"accepted"}]);
  });
  it("retains an open detail outside the page without inventing missing objects",()=>{
    expect(mergeReviewQueue([{id:"open"}],[{id:"queued"}],{keepId:"open"})).toEqual([{id:"queued"},{id:"open"}]);
    expect(mergeReviewQueue([],[{id:"queued"}],{newerDetailId:"missing"})).toEqual([{id:"queued"}]);
  });
  it("appends without duplicating or replacing existing entries",()=>{
    expect(mergeReviewQueue([{id:"a",value:2}],[{id:"a",value:1},{id:"b",value:3}],{append:true})).toEqual([{id:"a",value:2},{id:"b",value:3}]);
  });
});

describe("review detail hydration", () => {
  const baseline = { id: "item", attributes: {}, shape: "classification" };
  const detail = { ...baseline, shape: "polygon" };
  it("hydrates a pristine same-ID editor when exact detail arrives", () => {
    expect(canRefreshReviewDraft(baseline, detail, baseline, "{}", false)).toBe(true);
  });
  it("preserves geometry, attributes and unsaved decision input", () => {
    expect(canRefreshReviewDraft(baseline, detail, { ...baseline, shape: "edited" }, "{}", false)).toBe(false);
    expect(canRefreshReviewDraft(baseline, detail, baseline, '{"edited":true}', false)).toBe(false);
    expect(canRefreshReviewDraft(baseline, detail, baseline, "{}", true)).toBe(false);
  });
  it("loads a new selection and clears a missing selection", () => {
    expect(canRefreshReviewDraft(baseline, { ...detail, id: "other" }, detail, "{}", true)).toBe(true);
    expect(canRefreshReviewDraft(baseline, undefined, detail, "{}", true)).toBe(true);
  });
});
