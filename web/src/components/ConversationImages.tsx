import { useState } from "react";
import type { ImageItem } from "../types";

const PAGE_SIZE = 24;

/** Bounded thumbnail DOM over the existing dataset index; selection remains URL-owned. */
export function ConversationImages({images,selectedId,onSelect}:{images:ImageItem[];selectedId?:string;onSelect:(id:string)=>void}) {
  const [browsing,setBrowsing]=useState<{selection?:string;page:number}>();
  const selectedPage=Math.floor(Math.max(0,images.findIndex(image=>image.image_id===selectedId))/PAGE_SIZE);
  const lastPage=Math.max(0,Math.ceil(images.length/PAGE_SIZE)-1);
  const page=Math.min(lastPage,browsing && browsing.selection===selectedId ? browsing.page : selectedPage);
  const start=page*PAGE_SIZE;
  return <>
    {images.length>PAGE_SIZE && <nav className="button-row" aria-label="Image pages">
      <button disabled={page===0} onClick={()=>setBrowsing({selection:selectedId,page:page-1})}>Previous images</button>
      <span aria-live="polite">{start+1}–{Math.min(start+PAGE_SIZE,images.length)} of {images.length}</span>
      <button disabled={page===lastPage} onClick={()=>setBrowsing({selection:selectedId,page:page+1})}>Next images</button>
    </nav>}
    <nav key={page} className="conversation-thumbnails" aria-label="Select image">{images.slice(start,start+PAGE_SIZE).map(image=><button key={image.image_id} aria-label={image.name} aria-current={image.image_id===selectedId ? "true" : undefined} onClick={()=>onSelect(image.image_id)}><img loading="lazy" decoding="async" src={image.thumbnail_url ?? image.url} alt=""/><span>{image.name}</span></button>)}</nav>
  </>;
}
