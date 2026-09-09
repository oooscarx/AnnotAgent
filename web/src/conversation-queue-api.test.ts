import {expect,it} from "vitest";
import {parseQueueConsent} from "./conversation-queue-api";
const saved={call_id:"11111111-1111-4111-8111-111111111111",model_id:"22222222-2222-4222-8222-222222222222",scope_hash:"a".repeat(64),request_hash:"b".repeat(64),previous_grant_id:null,maximum_calls:2,expires_at:"2026-09-09T00:00:00Z",allow_unknown_cost:true};
it("restores exact pending consent including expired historical identity without extending it",()=>{
  expect(parseQueueConsent(JSON.stringify(saved))).toEqual(saved);
  for(const patch of [{call_id:"new"},{request_hash:""},{maximum_calls:0},{maximum_calls:2**32},{maximum_calls:1.5},{allow_unknown_cost:false},{execute:true},{expires_at:"unknown"}])expect(parseQueueConsent(JSON.stringify({...saved,...patch}))).toBeUndefined();
  for(const raw of [null,"{}","null","invalid"])expect(parseQueueConsent(raw)).toBeUndefined();
});
