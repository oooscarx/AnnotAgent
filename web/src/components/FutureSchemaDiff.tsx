import { futureSchemaDiff, type SchemaDefinition } from "../conversation-future-schema";

export function FutureSchemaDiff({ before, after }: { before: SchemaDefinition; after: SchemaDefinition }) {
  const changes = futureSchemaDiff(before, after);
  return <section className="conversation-future-schema-diff" aria-label="Future rule changes"><h4>Changes from the tested rules</h4>
    {!changes.length ? <p>No semantic changes yet.</p> : changes.map(change => <div key={change.field}><h5>{change.field}</h5><div className="conversation-future-schema-comparison"><div><strong>Tested rules</strong>{change.before.length ? <ul>{change.before.map((text, index) => <li key={index}>{text}</li>)}</ul> : <p>None</p>}</div><div><strong>Proposed rules</strong>{change.after.length ? <ul>{change.after.map((text, index) => <li key={index}>{text}</li>)}</ul> : <p>None</p>}</div></div>{change.added && <small>{change.added.length} added · {change.removed?.length ?? 0} removed{!change.added.length && !change.removed?.length ? " · Order changed" : ""}</small>}</div>)}
  </section>;
}
