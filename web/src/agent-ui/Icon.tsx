import type { ButtonHTMLAttributes } from "react";

export const iconNames = ["plus", "search", "folder", "chevron-left", "chevron-right", "chevron-down", "settings", "plan", "play", "stop", "pause", "arrow-up", "arrow-right", "close", "more", "check", "image", "panel", "undo", "fit", "download", "attachment", "pin", "edit", "alert", "globe", "server", "sun", "moon"] as const;
export type IconName = typeof iconNames[number];
export function Icon({ name, size = 18 }: { name: IconName; size?: 12 | 14 | 16 | 18 }) {
  if (!iconNames.includes(name)) throw new Error(`Unknown UI icon: ${name}`);
  return <svg className="ui-icon" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false"><use href={`/brand/core/ui-icons.svg#aa-${name}`} /></svg>;
}
export function IconButton({ icon, label, className = "", ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { icon: IconName; label: string }) {
  return <button type="button" {...props} className={`icon-button ${className}`} aria-label={label} title={label}><Icon name={icon} /></button>;
}
export function BrandMark() {
  return <span className="brand-mark" aria-hidden="true"><img className="mark-light" src="/brand/core/annotagent-mark-ink.svg" alt="" /><img className="mark-dark" src="/brand/core/annotagent-mark-paper.svg" alt="" /></span>;
}
