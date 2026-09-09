import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
/** Native modal provides focus containment; opening and closing preserve the originating control. */
export function Dialog({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const focus = document.activeElement as HTMLElement | null;
    const node = ref.current!;
    node.showModal();
    return () => {
      node.close();
      focus?.focus();
    };
  }, []);
  return (
    <dialog
      ref={ref}
      className="confirm-dialog native-dialog"
      aria-label={title}
      onCancel={(e) => {
        e.preventDefault();
        close.current();
      }}
    >
      <h2>{title}</h2>
      {children}
    </dialog>
  );
}
