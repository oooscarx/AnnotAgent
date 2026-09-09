import { useRef, type ReactNode, type RefObject } from "react";
import type { SendMode } from "../conversation-send";

/** Input mechanics only. The coordinator freezes mode, model and references;
 * server operation authorizations, not the selector, grant execution authority. */
export function AgentComposer({ inputRef, value, disabled, inputLocked, onChange, onSubmit, onCompositionChange, reference, actions, mode, onModeChange }: {
  inputRef: RefObject<HTMLTextAreaElement | null>;
  value: string;
  disabled: boolean;
  inputLocked: boolean;
  onChange: (value: string) => void;
  onSubmit: () => void;
  onCompositionChange: (composing: boolean) => void;
  reference: ReactNode;
  actions: ReactNode;
  mode: SendMode;
  onModeChange: (mode: SendMode) => void;
}) {
  const composing = useRef(false);
  return <form className="conversation-composer" aria-label="Agent composer" onSubmit={event => {
    event.preventDefault();
    if (!disabled && !composing.current) onSubmit();
  }}>
    {reference}
    <label className="sr-only" htmlFor="conversation-message">Your message</label>
    <textarea ref={inputRef} id="conversation-message" value={value} disabled={disabled || inputLocked} rows={3}
      placeholder="Find cups, but not bottles"
      onCompositionStart={() => { composing.current = true; onCompositionChange(true); }}
      onCompositionEnd={() => { composing.current = false; onCompositionChange(false); }}
      onChange={event => onChange(event.target.value)}
      onKeyDown={event => {
        if (event.key !== "Enter" || event.shiftKey || event.ctrlKey || event.metaKey || event.altKey || event.nativeEvent.isComposing || event.keyCode === 229 || composing.current) return;
        event.preventDefault();
        if (!disabled) onSubmit();
      }} />
    <div className="agent-composer-actions">
      <label className="agent-composer-mode"><span className="sr-only">Next message mode</span>
        <select value={mode} disabled={disabled||inputLocked} aria-describedby="agent-mode-scope" onChange={event=>onModeChange(event.target.value as SendMode)}>
          <option value="plan">Plan</option><option value="execute">Execute</option>
        </select>
      </label>
      {actions}
    </div>
    <small id="agent-mode-scope">{mode==="plan" ? "Plan: text-model fees may apply. No image processing without separate approval." : "Execute: review model, data and cost approval before processing. Changing mode does not start or stop work."}</small>
  </form>;
}
