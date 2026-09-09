import { useLayoutEffect, useRef, useState, type CSSProperties } from "react";

/** Keep the full accessible title; only its visual viewport is clipped. */
export function SidebarTitle({ children }: { children: string }) {
  const viewport = useRef<HTMLSpanElement>(null);
  const text = useRef<HTMLSpanElement>(null);
  const [overflow, setOverflow] = useState(0);
  useLayoutEffect(() => {
    const measure = () => setOverflow(Math.max(0, (text.current?.scrollWidth ?? 0) - (viewport.current?.clientWidth ?? 0)));
    measure();
    const observer = new ResizeObserver(measure);
    if (viewport.current) observer.observe(viewport.current);
    if (text.current) observer.observe(text.current);
    return () => observer.disconnect();
  }, [children]);
  return <span ref={viewport} className="sidebar-title" data-overflow={overflow > 0} style={{
    "--title-offset": `${-overflow}px`,
    "--title-duration": `${Math.min(8, Math.max(0.5, overflow / 55))}s`,
  } as CSSProperties}><span ref={text} className="sidebar-title-text">{children}</span></span>;
}
