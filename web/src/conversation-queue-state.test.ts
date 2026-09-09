import {expect,it} from "vitest";
import {canCancelQueuedMessage,isPendingQueuedMessage} from "./conversation-queue-state";

it("keeps waiting, authorized and unknown inbox items pending and cancellable",()=>{
  for(const status of ["waiting_for_dispatch","authorized","in_doubt"] as const){
    expect(isPendingQueuedMessage(status)).toBe(true);
    expect(canCancelQueuedMessage(status)).toBe(true);
  }
});
it("requires task stop for running calls and never cancels terminal inbox records",()=>{
  expect(isPendingQueuedMessage("running")).toBe(true);
  expect(canCancelQueuedMessage("running")).toBe(false);
  for(const status of ["completed","failed","cancelled"] as const){
    expect(isPendingQueuedMessage(status)).toBe(false);
    expect(canCancelQueuedMessage(status)).toBe(false);
  }
});
