# Schema structured-output hotfix

## Baseline and evidence

- Base: `725d13076a73d1a2d50a65f6a30c142771dedb40`.
- Reported failed logical call: `3a28d30a-0141-4acd-be40-1ae67edd814d`, model
  `glm-5.2`, 1,639 input tokens, 2,048 output tokens, null content and no tool calls.
- The source tree and repository documentation contain no saved raw response for that call.
  Therefore the 2,048 output count alone is not evidence that thinking consumed the complete
  limit. A saved `finish_reason=length` can confirm length termination; its absence remains an
  unknown cause even when the reported output count equals the cap.

The official Zhipu chat-completion reference declares `tool_choice=auto`, JSON Object as
`response_format:{"type":"json_object"}`, and the current thinking guide declares
`thinking:{"type":"disabled"}`. The hotfix identifies that dialect from the official
`*.bigmodel.cn` endpoint together with saved Model Profile capability flags. It never selects a
dialect merely from a model name containing `glm`.

References:

- https://docs.bigmodel.cn/api-reference/模型-api/对话补全
- https://docs.bigmodel.cn/cn/guide/capabilities/struct-output
- https://docs.bigmodel.cn/cn/guide/capabilities/thinking-mode
- https://docs.bigmodel.cn/cn/guide/capabilities/function-calling

## Request contract

`GET /api/projects/{project}/conversations/{conversation}/tasks/{task}/schema-preview`
continues to be passive. Its scope digest now freezes the server-resolved
`request_config` in addition to the Model and Provider snapshots:

```json
{
  "maximum_output_tokens": 4096,
  "response_mode": "json_object",
  "thinking": {
    "parameter": "thinking",
    "value": {"type": "disabled"}
  }
}
```

The same resolved values are used by authorization, request hashing, the Provider request and
the saved call evidence. An explicit lower Model Profile default remains lower. A higher value
changes the scope hash and needs a new preview/authorization. The Model Profile hard limit still
wins. Schema requests set Provider retries to zero.

Before this change, Application constructed `max_output_tokens=2048` and both direct and queued
Server paths separately applied `.min(2048)`. After this change the phase fallback is 4,096 and
the effective limit is `min(profile generation default or 4096, model limit)`.

For an official Zhipu endpoint with saved structured-output and reasoning capabilities, the
wire request is:

```json
{
  "model": "<saved remote model>",
  "max_tokens": 4096,
  "thinking": {"type": "disabled"},
  "response_format": {"type": "json_object"}
}
```

The JSON Object request omits `tools`, `tool_choice` and `json_schema`. It requests the fixed
`{"name":"propose_annotation_schema","arguments":{...}}` envelope. The adapter records
`action_source=json_adapter`; the shared `ConversationSchemaDecision` deserializer and validator
still enforce Draft/Clarify, unknown-field, label, type, length and delivery semantics. A native
tool request carries only the Schema tool and `tool_choice:auto`; it never emits `required` or a
named selector.

Provider and request extras are applied before protected fields are rebuilt, so extras cannot
replace model, messages, output cap, tools, tool choice, response format or the saved thinking
control.

## Saved result and failure evidence

Every HTTP-success response keeps Provider usage before Schema parsing. New evidence includes:

```json
{
  "diagnostic": {
    "finish_reason": "length",
    "content_present": false,
    "content_length": 0,
    "tool_call_count": 0,
    "reasoning_content_present": true,
    "reasoning_content_length": 9000,
    "reasoning_tokens": 1900,
    "actual_output_tokens": 2048,
    "maximum_output_tokens": 2048,
    "thinking_parameter": "thinking",
    "thinking_value": {"type": "disabled"},
    "response_mode": "json_object",
    "action_source": null,
    "failure_code": "length_terminated_without_action"
  }
}
```

Reasoning text is neither copied into metadata nor logged. Failure codes are:

- `length_terminated_without_action`: explicit Provider length finish reason and no action;
- `output_cap_observed_without_finish_reason`: output count equals the cap but no finish reason;
- `no_final_action`;
- `action_json_incomplete`;
- `schema_violation`;
- `wrong_action`;
- `multiple_actions`;
- `provider_filtered`.

An unsupported saved response mode fails before admission with HTTP 409
`schema_response_mode_unsupported`. Provider HTTP rejection remains a typed Provider failure with
its safe stage/category/status in the call receipt; it is separate from a completed but invalid
Schema response.

A completed Provider response with invalid structured output remains a settled call receipt with
its usage and a structured-output failure. It creates no Schema Draft and starts no image work.
Transport uncertainty keeps the existing `in_doubt` semantics and is never retried automatically.
Legacy saved Schema attempts without `diagnostic` remain readable through the defaulted field.

## Offline verification

- A loopback Rust HTTP Provider captures the final request and verifies JSON Object, disabled
  thinking, 4,096 cap, no conflicting tool fields, and resistance to `extra` overrides.
- Null content/no tools with explicit `finish_reason=length` keeps 1,639/2,048 usage and is
  classified as confirmed length termination.
- The same usage without finish reason uses the separate unconfirmed cap-observed code.
- Native legal tool calls and JSON-adapted envelopes pass the same strict business validator.
- Wrong/multiple actions, partial JSON and unknown fields remain non-executable while usage is
  retained and only one physical request occurs.
- Model Profile caps 1,024, 2,048 and 4,096 remain identical in stage Provider/request config.

No commercial Provider call was made. Real `glm-5.2` recovery remains unverified without a new
explicit cost authorization.

## Explicit recovery API

Recovery is a new authorization revision and a new physical call. It never mutates the original
receipt or reuses the original `call_id`.

### Passive preview

```http
GET /api/projects/{project}/conversations/{conversation}/tasks/{task}/schema-retry-preview?retry_of={failed_call}&model_id={same_model}
```

The GET has no side effects. A retry source must be a settled `completed` call whose saved
`decision` is invalid. `in_doubt`, cancelled, filtered/refused, valid and still-running calls are
rejected. The original Model Profile is frozen; changing Provider or model needs a separate task
decision.

```json
{
  "contract": "conversation-schema-retry-v1",
  "retry_of": "3a28d30a-0141-4acd-be40-1ae67edd814d",
  "source_status": "completed",
  "source_request_hash": "<sha256>",
  "source_diagnostic": {
    "failure_code": "length_terminated_without_action",
    "finish_reason": "length"
  },
  "model_id": "<model-profile-uuid>",
  "model_name": "glm-5.2",
  "destination": "https://open.bigmodel.cn",
  "previous_grant_id": "<current-grant-uuid>",
  "maximum_calls": 2,
  "new_request_limit": 1,
  "scope_hash": "<sha256>",
  "maximum_output_tokens": 4096,
  "response_mode": "json_object",
  "thinking": {"parameter":"thinking","value":{"type":"disabled"}},
  "estimated_cost": null,
  "expires_at": "<RFC3339, at most 30 minutes>",
  "operation": "Retry this saved Schema interpretation once with the corrected structured-output configuration. No image inference, publication or annotation acceptance."
}
```

### Explicit idempotent command

```http
POST /api/projects/{project}/conversations/{conversation}/tasks/{task}/schema-retries
Content-Type: application/json

{
  "call_id": "<new-command-and-call-uuid>",
  "retry_of": "3a28d30a-0141-4acd-be40-1ae67edd814d",
  "model_id": "<same-model-profile-uuid>",
  "previous_grant_id": "<preview.previous_grant_id>",
  "scope_hash": "<preview.scope_hash>",
  "maximum_calls": 2,
  "expires_at": "<preview.expires_at>",
  "allow_unknown_cost": true
}
```

The first accepted POST durably saves the exact command before Provider dispatch. Concurrent or
reconnected POSTs with the same `call_id` and identical body return the one reservation/receipt and
cannot dispatch a second request. Reusing the ID with changed scope returns HTTP 409
`schema_retry_command_conflict`. A changed preview, model, task snapshot, grant or expiry returns
HTTP 409 `schema_retry_scope_changed`; callers must review a fresh preview. The original call ID
always returns its original receipt.

The original failed receipt keeps its usage unchanged. Malformed native arguments are represented
as a non-executable null value plus safe counts; the partial raw argument string is not persisted.
Invalid JSON Object text is likewise discarded after its safe length and usage are captured.

```json
{
  "authorization": {"call_id":"<new-call>","retry_of":"<failed-call>","model_id":"<model>","previous_grant_id":"<grant>","scope_hash":"<sha256>","maximum_calls":2,"expires_at":"<RFC3339>","allow_unknown_cost":true},
  "receipt": {
    "id": "<new-call>",
    "status": "completed",
    "evidence": {
      "response": {"usage":{"input_tokens":200,"output_tokens":120,"total_tokens":320}},
      "decision": {"Ok":{"decision":"draft"}},
      "diagnostic": {"retry_of":"<failed-call>","response_mode":"json_object","maximum_output_tokens":4096,"thinking_parameter":"thinking","thinking_value":{"type":"disabled"}}
    }
  },
  "replayed": false,
  "journey_resume": {"journey_id":"<existing-journey-or-null>","queued":true}
}
```

`GET .../schema-retries/{call_id}` returns `{authorization,receipt}` and never executes. A successful
Draft linked to an existing Journey is queued into that same durable Journey. Its existing worker
saves the Draft and continues Builder/Sample under the already approved Journey scope. It does not
grant image access itself. Refresh and process restart observe the saved retry and Journey queue;
they do not resend the Provider request.

Migration `0072_conversation_schema_retries.sql` stores exact retry membership and enforces one
successor command per `(task_id,retry_of)`. Old calls and authorization rows remain unchanged.

## Remaining limits

- No commercial `glm-5.2` request was made, so the real endpoint path is not claimed verified.
- The recovery path supports a settled invalid Schema response from the direct Task/Journey Schema
  operation. It does not reinterpret a queued supplement as the original Task Schema.
- Provider rejection without a completed response remains `in_doubt` or typed Provider failure;
  it is not eligible for this repair command and is never automatically retried.
- The retry endpoint reports unknown estimated cost because no Provider quote is available. The
  original and retry attempts keep their own actual usage/cost records when the Provider reports it.
