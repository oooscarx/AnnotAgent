import { useRef, type ReactNode, type RefObject } from "react";

/** Input mechanics only. Authority and frozen command identity stay with the
 * existing conversation coordinator until the server send contract is connected. */
export function AgentComposer({ inputRef, value, disabled, inputLocked, onChange, onSubmit, onCompositionChange, reference, actions }: {
  inputRef: RefObject<HTMLTextAreaElement | null>;
  value: string;
  disabled: boolean;
  inputLocked: boolean;
  onChange: (value: string) => void;
  onSubmit: () => void;
  onCompositionChange: (composing: boolean) => void;
  reference: ReactNode;
  actions: ReactNode;
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
    <div className="agent-composer-actions">{actions}</div>
  </form>;
}
