import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { api } from "../api";
import { projectWorkPath } from "../navigation";
import type { ConversationMessage, ConversationMessageInput, ImageItem, ProjectSummary } from "../types";
import "./conversation-workspace.css";
import { ConversationSchemaCard } from "./ConversationSchemaCard";

/** The journal and image importer share the existing Project; neither starts inference. */
export function ConversationWorkspace({ project, conversationId, imageId, onNavigate, onNavigationGuardChange }: {
  project: ProjectSummary; conversationId?: string; imageId?: string;
  onNavigate: (path: string) => void;
  onNavigationGuardChange: (guard?: () => boolean) => void;
}) {
  const [conversation, setConversation] = useState<string>();
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const [images, setImages] = useState<ImageItem[]>([]);
  const [text, setText] = useState("");
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const [mobileView, setMobileView] = useState("conversation");
  const [width, setWidth] = useState(32);
  const root = useRef<HTMLDivElement>(null);
  const pending = useRef(false);
  const selectingImage = useRef(false);
  const unsent = useRef("");
  const schemaDirty = useRef(false);
  const schemaDirtyChange = useCallback((dirty: boolean) => { schemaDirty.current = dirty; }, []);
  const frozen = useRef<ConversationMessageInput | undefined>(undefined);
  const alive = useRef(true);
  const selected = images.find((image) => image.image_id === imageId) ?? (!imageId ? images[0] : undefined);
  const referenceImage = frozen.current ? images.find((image) => image.image_id === frozen.current?.image?.image_id) : selected;
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    const guard = () => selectingImage.current || (!pending.current && (!(unsent.current || schemaDirty.current) || window.confirm("Leave with unsaved message or Schema edits? Saved workspace data remains on the server.")));
    const unload = (event: BeforeUnloadEvent) => { if (pending.current || unsent.current || schemaDirty.current) event.preventDefault(); };
    onNavigationGuardChange(guard);
    window.addEventListener("beforeunload", unload);
    return () => { onNavigationGuardChange(undefined); window.removeEventListener("beforeunload", unload); };
  }, [onNavigationGuardChange]);
  useEffect(() => {
    const controller = new AbortController();
    setReady(false); setError("");
    void (async () => {
      const [current, dataset] = await Promise.all([api.conversation(project.id, controller.signal), api.images(project.id, controller.signal)]);
      if (conversationId && conversationId !== current.conversation_id) throw new Error("This conversation does not belong to this Project or is no longer available.");
      const saved: ConversationMessage[] = [];
      if (current.conversation_id) {
        let page: ConversationMessage[];
        do {
          page = await api.conversationMessages(project.id, current.conversation_id, saved.at(-1)?.sequence ?? 0, controller.signal);
          saved.push(...page);
        } while (page.length === 100);
      }
      if (controller.signal.aborted) return;
      setConversation(current.conversation_id ?? undefined); setImages(dataset.images); setMessages(saved); setReady(true);
    })().catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    return () => controller.abort();
  }, [project.id, conversationId]);
  async function send() {
    if (pending.current || !ready || !text.trim()) return;
    frozen.current ??= { id: crypto.randomUUID(), text, image: selected ? { image_id: selected.image_id, sha256: selected.content_hash } : null };
    const input = frozen.current;
    pending.current = true; setBusy(true); setError(""); setStatus("Saving message…");
    try {
      const id = conversation ?? (await api.createConversation(project.id)).conversation_id;
      const saved = await api.sendConversationMessage(project.id, id, input);
      if (!alive.current) return;
      setConversation(id); setMessages((items) => [...items.filter((item) => item.input.id !== saved.input.id), saved].sort((a, b) => a.sequence - b.sequence));
      frozen.current = undefined; unsent.current = ""; setText(""); setStatus("Message saved. No model has been called.");
    } catch (error) { if (alive.current) { setError((error as Error).message); setStatus("Not confirmed saved. Retry sends the same message and frozen image reference."); } }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function upload(files: File[]) {
    if (pending.current || !files.length) return;
    pending.current = true; setBusy(true); setError("");
    try {
      for (const [index, file] of files.entries()) {
        setStatus(`Uploading ${index + 1}/${files.length} to this AnnotAgent server…`);
        const result = await api.uploadImage(project.id, file);
        if (result.corrupt.length) throw new Error(result.corrupt.map((item) => `${item.name}: ${item.message}`).join("; "));
      }
      const dataset = await api.images(project.id);
      if (alive.current) { setImages(dataset.images); setStatus("Images saved on this server. No model has been called."); }
    } catch (error) { if (alive.current) { setError((error as Error).message); setStatus("Completed uploads are retained. Reselect files to retry; identical content is deduplicated."); } }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  const openImage = (id: string) => {
    selectingImage.current = true;
    try { onNavigate(projectWorkPath(project.id, { conversationId: conversation, imageId: id })); }
    finally { selectingImage.current = false; }
  };
  return <section className="conversation-workspace" aria-label="Annotation workspace">
    <nav className="conversation-mobile-tabs" aria-label="Workspace panels">
      <button aria-pressed={mobileView === "conversation"} onClick={() => setMobileView("conversation")}>Conversation</button>
      <button aria-pressed={mobileView === "images"} onClick={() => setMobileView("images")}>Images ({images.length})</button>
    </nav>
    <div ref={root} className="conversation-split" data-mobile-view={mobileView} style={{ "--conversation-width": `${width}%` } as CSSProperties}>
      <section className="conversation-panel" aria-label="Project conversation">
        <h2>What would you like to annotate?</h2>
        <p className="muted">Describe your goal before or after uploading images.</p>
        <p className="conversation-development-note">Workspace integration in progress: propose labels and build a Pipeline Draft after authorization. Sample execution and annotation editing are not connected here yet.</p>
        <ol className="conversation-messages" aria-label="Saved messages">
          {messages.map((message) => <li key={message.input.id}><p>{message.input.text}</p>{message.input.image && <button onClick={() => {
            const reference = message.input.image;
            const image = images.find((item) => item.image_id === reference?.image_id);
            if (!reference || !image || image.content_hash !== reference.sha256) { setError("The referenced image was removed or changed. Its historical reference remains saved; current pixels cannot stand in for that evidence."); return; }
            openImage(image.image_id);
          }}>Referenced image · {images.find((image) => image.image_id === message.input.image?.image_id)?.name ?? message.input.image.image_id}</button>}<small>Saved · {message.sequence}</small></li>)}
        </ol>
        {conversation && messages[0] && <ConversationSchemaCard key={`${conversation}:${messages[0].input.id}`} project={project.id} conversation={conversation} message={messages[0].input.id} onDirtyChange={schemaDirtyChange} />}
        <form onSubmit={(event) => { event.preventDefault(); void send(); }} className="conversation-composer">
          <label htmlFor="conversation-message">Your message</label>
          <textarea id="conversation-message" value={text} disabled={busy || Boolean(frozen.current)} rows={3} placeholder="Find cups, but not bottles" onChange={(event) => { unsent.current = event.target.value; setText(event.target.value); }} />
          <small>{referenceImage ? `Image reference: ${referenceImage.name}` : "No image reference · Project-level message"}</small>
          <button className="primary" disabled={!ready || busy || !text.trim()} type="submit">{busy ? "Saving…" : frozen.current ? "Retry saving message" : "Save message"}</button>
        </form>
      </section>
      <div className="conversation-divider" role="separator" aria-label="Resize conversation panel" aria-orientation="vertical" tabIndex={0} aria-valuemin={25} aria-valuemax={50} aria-valuenow={width}
        onKeyDown={(event) => { if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); setWidth((current) => Math.max(25, Math.min(50, current + (event.key === "ArrowRight" ? 2 : -2)))); } }}
        onPointerDown={(event) => event.currentTarget.setPointerCapture(event.pointerId)}
        onPointerMove={(event) => { if (!event.currentTarget.hasPointerCapture(event.pointerId)) return; const bounds = root.current?.getBoundingClientRect(); if (bounds) setWidth(Math.round(Math.max(25, Math.min(50, (event.clientX - bounds.left) / bounds.width * 100)))); }} />
      <section className="conversation-image-panel" aria-label="Project images">
        <div className="conversation-image-tools"><h2>{images.length ? `${images.length} images` : "Your images"}</h2><label className="conversation-upload">Add images<input type="file" accept="image/png,image/jpeg" multiple disabled={busy || !ready} onChange={(event) => { const files = Array.from(event.currentTarget.files ?? []); event.currentTarget.value = ""; void upload(files); }} /></label></div>
        {imageId && !selected && ready ? <p role="alert">This image is not available in this Project. Select an existing image below.</p> : selected ? <figure className="conversation-image"><img src={selected.url} alt={selected.name} /><figcaption>{selected.name} · Original image · No annotation overlay</figcaption></figure> : <div className="conversation-image-empty"><h3>Start with your own images</h3><p>PNG or JPEG · up to 25 MB per image. Files are uploaded to this AnnotAgent server, not sent to a model.</p></div>}
        <nav className="conversation-thumbnails" aria-label="Select image">{images.map((image) => <button key={image.image_id} aria-label={image.name} aria-current={image.image_id === selected?.image_id ? "true" : undefined} onClick={() => openImage(image.image_id)}><img loading="lazy" src={image.url} alt="" /><span>{image.name}</span></button>)}</nav>
      </section>
    </div>
    <p className="conversation-status" role="status">{status || (ready ? "Saved workspace loaded" : "Loading saved workspace…")}</p>
    {error && <p role="alert">{error}</p>}
  </section>;
}
