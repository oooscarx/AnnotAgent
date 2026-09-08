import type {ProjectCallLimitSnapshot} from "./types";

export function projectBudgetAvailability(value?:ProjectCallLimitSnapshot){
  if(!value || !Number.isSafeInteger(value.revision) || value.revision<0 || !Number.isSafeInteger(value.reserved_calls) || value.reserved_calls<0 || (value.maximum_calls!==null&&(!Number.isSafeInteger(value.maximum_calls)||value.maximum_calls<0)))return {blocked:true,remaining:null,known:false};
  const remaining=value.maximum_calls===null?null:Math.max(0,value.maximum_calls-value.reserved_calls);
  return {blocked:remaining===0,remaining,known:true};
}
