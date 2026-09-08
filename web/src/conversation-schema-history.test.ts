import { describe, expect, it } from "vitest";
import type { ConversationBuilderItem } from "./types";
import type { JourneyConsent, JourneyStatus } from "./api";
import { builderMatchesSchema, journeyMatchesSchema, consentMatchesSchema, consentMatchesRepair } from "./conversation-schema-history";

const schema = { id: "TEST-new-schema", revision: 1 };
describe("exact Schema history ownership", () => {
  it("isolates normal plans and each exact correction's joint authorization", () => {
    const repair = {id:"TEST-request",draft:"TEST-draft"};
    const consent = {repair:{request_id:repair.id,draft_id:repair.draft,revision:2,content_hash:"TEST"}};
    expect(consentMatchesRepair(consent,repair)).toBe(true);
    expect(consentMatchesRepair(consent)).toBe(false);
    expect(consentMatchesRepair({},repair)).toBe(false);
    expect(consentMatchesRepair(consent,{...repair,id:"other"})).toBe(false);
    expect(consentMatchesRepair(consent,{...repair,draft:"other"})).toBe(false);
    expect(consentMatchesRepair({})).toBe(true);
  });
  it("does not reuse another Schema's revision one build", () => {
    const item = { schema_id: "TEST-old-schema", schema_revision: 1, operation: { evidence: { schema_id: "TEST-old-schema", schema_revision: 1 } } } as ConversationBuilderItem;
    expect(builderMatchesSchema(item, schema)).toBe(false);
    expect(builderMatchesSchema({ ...item, schema_id: schema.id }, schema)).toBe(false);
    expect(builderMatchesSchema({ operation: { evidence: { schema_id: schema.id, schema_revision: 1 } } } as ConversationBuilderItem, schema)).toBe(true);
    expect(builderMatchesSchema({ schema_revision: 1, operation: {} } as ConversationBuilderItem, schema)).toBe(false);
  });
  it("uses resolved exact journey identity, never its revision number alone", () => {
    const consent = { schema_id: "TEST-old-schema", schema_revision: 1 } as JourneyConsent;
    const item = { record: { consent } } as JourneyStatus;
    expect(journeyMatchesSchema(item, schema)).toBe(false);
    expect(journeyMatchesSchema({ ...item, record: { ...item.record, resolved_consent: { ...consent, schema_id: schema.id } } }, schema)).toBe(true);
    expect(consentMatchesSchema(consent, schema)).toBe(false);
    expect(consentMatchesSchema({ ...consent, schema_id: schema.id, schema_revision: 2 }, schema)).toBe(false);
  });
  it("only reconnects the exact revision-one clarification draft before resolution", () => {
    const item = { record: { consent: { schema_proposal: {}, continue_after_clarification: true } }, clarification: { schema_draft_id: schema.id, schema_revision: 1 } } as JourneyStatus;
    expect(journeyMatchesSchema(item, schema)).toBe(true);
    expect(journeyMatchesSchema(item, { ...schema, revision: 2 })).toBe(false);
    expect(journeyMatchesSchema(item, { ...schema, id: "TEST-unrelated" })).toBe(false);
    expect(journeyMatchesSchema({ ...item, clarification: { schema_draft_id: schema.id, status: "applied" } }, schema)).toBe(false);
  });
});
