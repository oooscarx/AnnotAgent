/** Merge list data without letting an older in-flight list erase a newer detail. */
export function mergeReviewQueue<T extends {id:string}>(current:T[],incoming:T[],options:{append?:boolean;keepId?:string;newerDetailId?:string}):T[]{
  if(options.append)return [...current,...incoming.filter(value=>!current.some(item=>item.id===value.id))];
  const detail=current.find(value=>value.id===options.newerDetailId);
  const rows=incoming.map(value=>detail?.id===value.id?detail:value);
  return [...rows,...current.filter(value=>(value.id===options.keepId||value.id===detail?.id)&&!rows.some(item=>item.id===value.id))];
}

/** A detail arriving for the same object may refresh only an untouched editor. */
export function canRefreshReviewDraft<T extends { id: string; attributes?: Record<string, unknown> }>(
  previous: T | undefined,
  next: T | undefined,
  draft: T | undefined,
  attributesText: string,
  hasLocalDecision: boolean,
): boolean {
  if (!previous || previous.id !== next?.id) return true;
  return !hasLocalDecision && JSON.stringify(draft) === JSON.stringify(previous) &&
    attributesText === JSON.stringify(previous.attributes ?? {}, null, 2);
}
