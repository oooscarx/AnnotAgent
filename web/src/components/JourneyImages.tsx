import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import type { ImageItem, ProjectSummary } from "../types";

/** Uploads use the same Project and image importer as Data management. */
export function JourneyImages({ project, onContinue, onNavigationGuardChange }: {
  project?: ProjectSummary;
  onContinue: (id: string) => Promise<void>;
  onNavigationGuardChange: (guard?: () => boolean) => void;
}) {
  const [files, setFiles] = useState<File[]>([]);
  const [previews, setPreviews] = useState<string[]>([]);
  const [images, setImages] = useState<ImageItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [progress, setProgress] = useState("");
  const identity = useRef(project?.id ?? `project-${crypto.randomUUID()}`);
  const created = useRef(Boolean(project));
  const finished = useRef(false);
  const pending = useRef(false);
  useEffect(() => {
    const urls = files.map((file) => URL.createObjectURL(file));
    setPreviews(urls);
    return () => urls.forEach((url) => URL.revokeObjectURL(url));
  }, [files]);
  useEffect(() => {
    if (!project) return;
    const controller = new AbortController();
    void api.images(project.id, controller.signal).then((value) => setImages(value.images)).catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    return () => controller.abort();
  }, [project?.id]);
  useEffect(() => {
    const guard = () => finished.current || (!pending.current && (!files.length || window.confirm(t("Discard locally selected images? Uploaded images will remain saved."))));
    onNavigationGuardChange(guard);
    const unload = (event: BeforeUnloadEvent) => { if (!finished.current && (pending.current || files.length)) event.preventDefault(); };
    window.addEventListener("beforeunload", unload);
    return () => { onNavigationGuardChange(undefined); window.removeEventListener("beforeunload", unload); };
  }, [files.length, onNavigationGuardChange]);
  async function uploadAndContinue() {
    if (pending.current || (!files.length && !images.length)) return;
    pending.current = true; setBusy(true); setError("");
    try {
      if (!created.current) {
        const name = files[0]?.name.replace(/\.[^.]+$/, "") || "Images";
        await api.createProject(identity.current, `version: 1\nproject:\n  name: ${JSON.stringify(name)}\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n`);
        created.current = true;
      }
      for (const [index, file] of files.entries()) {
        setProgress(t("Uploading image {current} of {total}", { current: index + 1, total: files.length }));
        const result = await api.uploadImage(identity.current, file);
        if (result.corrupt.length) throw new Error(result.corrupt.map((item) => `${item.name}: ${item.message}`).join("; "));
      }
      setFiles([]); finished.current = true;
      await onContinue(identity.current);
    } catch (error) {
      finished.current = false;
      setError((error as Error).message);
      setProgress(t("Saved uploads are retained. Retry skips identical image content."));
    } finally { pending.current = false; setBusy(false); }
  }
  return <section className="journey-scene" aria-label={t("Create Project")}>
    <div className="journey-intro"><h2>{t("Start with the images you want to annotate.")}</h2><p>{t("Choose your own images. We will define what to find next.")}</p></div>
    <label className="journey-upload"><span>{t("Choose images")}</span><input aria-label={t("Choose images")} type="file" accept="image/png,image/jpeg" multiple disabled={busy} onChange={(event) => { const selected = Array.from(event.currentTarget.files ?? []); setFiles((current) => [...current, ...selected]); event.currentTarget.value = ""; }} /><small>{t("PNG or JPEG · up to 25 MB per image · uploaded to this AnnotAgent server")}</small></label>
    {(files.length > 0 || images.length > 0) && <div className="journey-image-grid">
      {images.map((image) => <figure key={image.image_id}><img src={image.url} alt={image.name} /><figcaption>{image.name}<small>{t("Saved")}</small></figcaption></figure>)}
      {files.map((file, index) => <figure key={`${file.name}:${index}`}><img src={previews[index]} alt={file.name} /><figcaption>{file.name}<button disabled={busy} aria-label={t("Remove selected image {name}", { name: file.name })} onClick={() => setFiles((items) => items.filter((_, position) => position !== index))}>{t("Remove")}</button></figcaption></figure>)}
    </div>}
    <p role="status">{progress || (files.length ? t("These previews are not uploaded yet. Continue saves them; reloading now requires reselecting the files.") : images.length ? t("Your images are saved on this server.") : t("No images uploaded yet."))}</p>
    {error && <p role="alert">{error}</p>}
    <footer className="journey-actions"><span>{t("No model is called on this page.")}</span><button className="primary" disabled={busy || (!files.length && !images.length)} onClick={() => void uploadAndContinue()}>{t(busy ? "Saving images…" : "Continue")}</button></footer>
  </section>;
}
