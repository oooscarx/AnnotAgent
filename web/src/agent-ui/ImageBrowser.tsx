import {useState} from "react";
import type {ImageId,Snapshot} from "./adapter";
const pageSize=24;
export function ImageBrowser({assets,image,onSelect}:{assets:Snapshot["artifacts"];image:ImageId;onSelect:(id:ImageId)=>void}){
  const [browsing,setBrowsing]=useState<{image:ImageId;page:number}>();
  const selectedPage=Math.floor(Math.max(0,assets.findIndex(a=>a.id===image))/pageSize);
  const lastPage=Math.max(0,Math.ceil(assets.length/pageSize)-1);
  const page=Math.min(lastPage,browsing?.image===image?browsing.page:selectedPage);
  const start=page*pageSize;
  return <section aria-label="图片浏览">
    {assets.length>pageSize&&<nav className="artifact-toolbar" aria-label="图片分页"><button disabled={page===0} onClick={()=>setBrowsing({image,page:page-1})}>上一页图片</button><span role="status">{start+1}–{Math.min(start+pageSize,assets.length)} / {assets.length}</span><button disabled={page===lastPage} onClick={()=>setBrowsing({image,page:page+1})}>下一页图片</button></nav>}
    <div className="thumbnails">{assets.slice(start,start+pageSize).map(a=><button key={a.id} aria-label={`查看图片 ${a.id}`} title={a.name} aria-pressed={a.id===image} onClick={()=>onSelect(a.id)}><img src={a.thumbnail||a.src} alt="" loading="lazy" decoding="async"/></button>)}</div>
  </section>;
}
