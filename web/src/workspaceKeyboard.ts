/** Canvas shortcuts must not consume text editing, composition or dialog input. */
export function workspaceShortcutAllowed(event: {
  isComposing?: boolean;
  keyCode?: number;
  defaultPrevented?: boolean;
}, editingControl: boolean, dialogOpen: boolean): boolean {
  return !event.isComposing && event.keyCode !== 229 && !event.defaultPrevented
    && !editingControl && !dialogOpen;
}

export function isTextEditingTarget(target: EventTarget | null): boolean {
  return target instanceof Element && Boolean(target.closest(
    "input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='textbox']",
  ));
}
