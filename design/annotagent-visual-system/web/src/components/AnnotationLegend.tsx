export type AnnotationLegendItem = {
  id: string;
  label: string;
  color: string;
  count?: number;
  shape?: "box" | "line" | "point" | "polygon" | "mask";
};

export function AnnotationLegend({ items }: { items: AnnotationLegendItem[] }) {
  return (
    <ul aria-label="Annotation legend" style={{ display: "grid", gap: 8, margin: 0, padding: 0, listStyle: "none" }}>
      {items.map((item) => (
        <li key={item.id} style={{ display: "flex", alignItems: "center", gap: 9, minHeight: 28 }}>
          <span
            aria-hidden="true"
            style={{ width: 12, height: 12, borderRadius: item.shape === "point" ? "50%" : 3, border: `2px solid ${item.color}`, background: item.shape === "mask" ? `${item.color}33` : "transparent" }}
          />
          <span style={{ flex: 1 }}>{item.label}</span>
          {typeof item.count === "number" && <span className="aa-mono aa-muted">{item.count}</span>}
        </li>
      ))}
    </ul>
  );
}
