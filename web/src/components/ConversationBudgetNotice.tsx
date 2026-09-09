import type {ProjectCallLimitSnapshot} from "../types";
import {projectBudgetAvailability} from "../projectBudget";
import { createContext, useContext } from "react";

export const ProjectBudgetAccess = createContext<(() => void) | undefined>(undefined);

/** Read-only snapshot. It neither changes the ceiling nor replaces server admission. */
export function ConversationBudgetNotice({value,maximumCalls,busy,onRefresh}:{value?:ProjectCallLimitSnapshot;maximumCalls:number;busy:boolean;onRefresh:()=>void}){
  const budget=projectBudgetAvailability(value);
  const openBudget = useContext(ProjectBudgetAccess);
  return <aside className="conversation-consent" aria-label="Project budget before inference">
    <p role={budget.blocked?"status":undefined}>{!budget.known?"Project budget snapshot is unavailable. Refresh this authorization before starting a new model request.":budget.remaining===null?`Project has ${value!.reserved_calls} reserved calls and no shared ceiling configured. This task's explicit call limit still applies.`:budget.blocked?`Project call limit exhausted: ${value!.reserved_calls} calls reserved, ${value!.maximum_calls} cumulative maximum. No new model request can start.`:`Project has ${budget.remaining} calls remaining (${value!.reserved_calls} reserved of ${value!.maximum_calls}).`}</p>
    {budget.remaining!==null && budget.remaining>0 && budget.remaining<maximumCalls && <p>This step permits up to {maximumCalls} calls, more than the Project currently has left. It may stop at the shared limit; that limit will not be raised automatically.</p>}
    <small>Snapshot only. Other tasks may consume the remaining calls. Setting a Project limit does not grant model or image permissions.</small>
    {budget.blocked&&<p>To change the shared ceiling, open Project call limit in this workspace, confirm a cumulative value, then refresh this authorization.</p>}
    {budget.blocked&&<button disabled={busy} onClick={event=>{
      if (openBudget) { openBudget(); return; }
      const panel=event.currentTarget.closest(".conversation-workspace")?.querySelector<HTMLDetailsElement>("details.conversation-project-budget");
      if(panel){panel.open=true;panel.querySelector<HTMLInputElement>("input[type=number]")?.focus();panel.scrollIntoView({block:"nearest"});}
    }}>Review Project call limit</button>}
    <button disabled={busy} onClick={onRefresh}>Refresh authorization and Project budget</button>
  </aside>;
}
