import { useState } from "react";
import { Icon, IconButton, iconNames } from "../src/agent-ui/Icon";
export function IconGallery() {
  const [dark,setDark]=useState(false);
  return <div data-aa-theme={dark?"dark":"light"}><main className="ui-app icon-gallery">
    <header><h1>IconGallery · UI 预览</h1><p>第一版 SVG 素材 · 无 API 和模型调用。16 / 18px；Tab 检查焦点，悬停检查 hover，灰色为 disabled。</p><button onClick={()=>setDark(!dark)}>切换浅色 / 深色</button></header>
    <div className="icon-gallery-grid">{iconNames.map(name=><section key={name}><strong>{name}</strong><div><Icon name={name} size={16}/><Icon name={name} size={18}/><IconButton icon={name} label={`${name} active`}/><IconButton icon={name} label={`${name} disabled`} disabled/></div></section>)}</div>
  </main></div>;
}
