import type { JourneyConsent, JourneyStatus } from "./api";
import type { ConversationBuilderItem } from "./types";

type SchemaIdentity = { id: string; revision: number };
export function consentMatchesSchema(consent: Pick<JourneyConsent, "schema_id" | "schema_revision">, schema: SchemaIdentity) {
  return consent.schema_id === schema.id && consent.schema_revision === schema.revision;
}
export function builderMatchesSchema(item: ConversationBuilderItem, schema: SchemaIdentity) {
  const evidence = item.operation.evidence;
  // Conflicting identity is not evidence of a match, even if one field is correct.
  if (item.schema_id && evidence?.schema_id && item.schema_id !== evidence.schema_id) return false;
  if (item.schema_revision && evidence?.schema_revision && item.schema_revision !== evidence.schema_revision) return false;
  return (item.schema_id ?? evidence?.schema_id) === schema.id && (item.schema_revision ?? evidence?.schema_revision) === schema.revision;
}
/** Class repairs require the admitted source, including before a working Draft exists. */
export function builderMatchesClassRepair(item: ConversationBuilderItem, schema: SchemaIdentity, review: {id:string;draft:string}) {
  const source=item.operation.evidence?.repair_source;
  return builderMatchesSchema(item,schema) && source?.kind==="image_class_review" && source.reference.review_id===review.id && source.reference.draft_id===review.draft && source.reference.schema_id===schema.id && source.reference.schema_revision===schema.revision;
}
export function journeyMatchesSchema(item: JourneyStatus, schema?: SchemaIdentity) {
  if (!schema) return true;
  if (item.record.resolved_consent) return consentMatchesSchema(item.record.resolved_consent, schema);
  if (consentMatchesSchema(item.record.consent, schema)) return true;
  // The server supplies the clarification's owned creation revision, not latest.
  return Boolean(item.record.consent.schema_proposal && item.record.consent.continue_after_clarification && item.clarification?.schema_draft_id === schema.id && item.clarification.schema_revision === schema.revision);
}
