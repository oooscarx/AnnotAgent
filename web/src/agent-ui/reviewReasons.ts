import type {SkillDetail} from "../types";
export const commonReviewReasons = [["too_loose","边界太大"],["too_tight","边界太小"],["shifted","位置偏移"],["wrong_object","目标找错"],["missed_object","有遗漏"],["duplicate","重复目标"],["wrong_label","标签错误"],["other","其他问题"]] as const;
export type ReviewReasonOption={key:string;code:string;label:string;skillId?:string};
export function reviewReasonOptions(skills:SkillDetail[],enabledIds:string[]):ReviewReasonOption[]{
  return [
    ...commonReviewReasons.map(([code,label])=>({key:code,code,label})),
    ...skills.filter(s=>enabledIds.includes(s.id)).flatMap(s=>[...new Set(s.correction_taxonomy)].map(code=>({
      key:JSON.stringify([s.id,code]),code,label:`${s.display_name} · ${code.replaceAll("_"," ")}`,skillId:s.id,
    }))),
  ];
}
export function reviewDecisionReason(options:ReviewReasonOption[],key:string,decision:"accept"|"reject",sourceSkill:string|undefined,enabledIds:string[]){
  const option=options.find(value=>value.key===key);
  if(decision==="reject"&&!option)throw new Error("所选审核原因已不可用，请重新选择。未提交审核决定。");
  return {code:decision==="accept"?"accepted_as_is":option!.code,skillId:decision==="reject"&&option?.skillId?option.skillId:sourceSkill&&enabledIds.includes(sourceSkill)?sourceSkill:undefined};
}
