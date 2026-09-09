import { useEffect, useRef, type ReactNode } from "react";
import { Icon } from "./Icon";
export function ProjectMenu({children}: {children: ReactNode}) {
  const root = useRef<HTMLDetailsElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  useEffect(()=>{
    const close=(event:PointerEvent)=>{if(root.current?.open && !root.current.contains(event.target as Node))root.current.open=false;};
    const escape=(event:KeyboardEvent)=>{if(event.key==="Escape" && root.current?.open){root.current.open=false;root.current.querySelector("summary")?.focus();}};
    document.addEventListener("pointerdown",close);document.addEventListener("keydown",escape);
    return()=>{document.removeEventListener("pointerdown",close);document.removeEventListener("keydown",escape);};
  },[]);
  return <details ref={root} className="project-menu-anchor" onToggle={()=>{
    if(!root.current?.open || !panel.current)return;
    const trigger=root.current.getBoundingClientRect();
    panel.current.style.left=`${Math.max(8,Math.min(trigger.right-panel.current.offsetWidth,innerWidth-panel.current.offsetWidth-8))}px`;
    panel.current.style.top=`${Math.max(8,Math.min(trigger.bottom+6,innerHeight-panel.current.offsetHeight-8))}px`;
  }}><summary aria-label="项目管理菜单" title="项目管理菜单"><Icon name="more" /></summary><div ref={panel} className="project-menu">{children}</div></details>;
}
