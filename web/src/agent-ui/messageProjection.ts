import type {ConversationCallReceipt} from "../types";
import type {ThreadItem} from "./adapter";

/** Only persisted provider decisions become Agent messages. Empty/invalid receipts stay operation records. */
export function projectCallMessages(calls:ConversationCallReceipt[]):ThreadItem[]{
  return calls.flatMap(call=>{
    const decision=call.evidence?.decision;
    if(!decision||!("Ok" in decision))return [];
    const value=decision.Ok;
    if(!value)return [];
    const text=(value.question||value.rationale||"").trim();
    if(!text)return [];
    const details=[value.question&&value.rationale!==value.question?value.rationale:"",value.labels?.length?`标签：${value.labels.join("、")}`:""].filter(Boolean) as string[];
    return [{id:`agent:${call.id}`,role:"assistant" as const,kind:value.decision==="clarify"?"clarification" as const:"reply" as const,text,details,source:{kind:"model_call" as const,id:call.id,status:call.status}}];
  });
}
