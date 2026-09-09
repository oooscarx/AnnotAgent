import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { Icon } from "./Icon";

/** Native disclosure semantics, one shared SVG indicator, no nested toggle. */
export function Disclosure({ title, children, className = "", ...props }: Omit<ComponentPropsWithoutRef<"details">, "title"> & { title: ReactNode }) {
  return <details {...props} className={`ui-disclosure ${className}`}>
    <summary><span className="disclosure-chevron"><Icon name="chevron-right" size={14} /></span><span className="disclosure-title">{title}</span></summary>
    {children}
  </details>;
}
