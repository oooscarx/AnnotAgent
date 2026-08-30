# Lean Agent Alpha Decisions

## D001 — Preserve runtime, narrow the product

The existing Published DAG executor, Artifact model, Batch coordinator, Review and Replay are the
baseline. Lean work changes authoring vocabulary and Agent ownership; it does not introduce a
parallel executor.

## D002 — Capability Skills are generic

Classification, Detection and Segmentation are public Capability Skills. Qwen, YOLO, RF-DETR,
LocateAnything, SAM and Mock are Model Backends selected through Model Descriptors. Legacy
brand-specific Skill crates remain compatibility adapters until persisted references are migrated.

## D003 — One visible Agent

Pipeline Builder Agent is the only user-visible Agent in Alpha. Existing runtime Detection Recovery
is presented and evolved as deterministic Fallback Policy because it only executes published,
bounded conditions.

## D004 — Tool calls own mutations

The model may select only Registry-defined Pipeline Builder tools. Rust Application services
validate arguments and perform mutations. The model cannot write a Workflow JSON directly, access
SQLite, execute code, invoke Shell or open an arbitrary URL.

## D005 — Human approval remains explicit

The Agent may create/revise/test a Draft and submit it for approval. It cannot Publish or start a
formal Run. Published Versions remain immutable.

## D006 — Unavailable backends are Labs

Model configuration and health determine recommendation eligibility. A registered but unhealthy or
unconfigured Worker is visible only in Labs/alternatives and blocks publish if left unresolved.

## D007 — Compatibility aliases are Registry-only

Pre-Lean IDs remain registered so stored Projects and immutable versions resolve, but their
manifests are marked `compatibility` with a canonical replacement. The public `/api/skills` catalog
filters them. New examples and authoring use generic Capability IDs.

## D008 — Segmentation can be unavailable without being fake

The generic Segmentation Capability is a real semantic contract but publishes no runnable node or
template until a compatible Model Backend is healthy. SAM remains a Labs Model Binding and the
existing RoboCup adapter is not presented as a general ready backend.

## D009 — Guided vocabulary is a projection, not a second graph

Select detections, Decision and Combine model evidence group adjacent technical nodes for ordinary
authoring. The persisted Workflow and Runtime keep their typed Filter, Map Label, match, attach and
gate nodes. Expert details expose those identities when debugging; Guided actions never rewrite or
silently discard them.

## D010 — Grounding assistance belongs to Detection configuration

Grid assistance is the bounded `grounding_assist` configuration of a Detection step. The provider
receives the unmodified source image first and an optional generated calibration view second. The
legacy `localization_grid` parameter is read only for published-version compatibility.

## D011 — Agent tools form a closed protocol

Pipeline Builder accepts only the versioned Rust Registry of 31 tools. Unknown names fail before
an Application action runs. Shell, code execution, Python, package installation, model download and
arbitrary URL access are not represented by the protocol and are covered by rejection tests.

## D012 — Intermediate Drafts may be invalid, mutations may not escape the Registry

An Agent is allowed to create a structurally incomplete editable Draft so static validation can
guide repair. It cannot introduce unknown node/model/Skill identities, type-invalid connections,
cycles, or mutate Published/Archived content. The ScriptedMock creates its first error by removing a
real connection through the same bounded mutation service, not by inventing a fake model ID.

## D013 — ScriptedMock is a policy, not fake inference

ScriptedMock deterministically chooses the full tool sequence and supplies labelled mock evaluation
observations for CI/course demonstrations. Rust still validates and records every tool. It is never
presented as a real visual-model result; M5 binds the same phases to real sandbox summaries.

## D014 — Provider output selects actions, never owns state

The live provider sees bounded Tool schemas and model-facing Tool results. It cannot submit a whole
Workflow document. Application services own the current Draft, Registry checks, validation, Dry
Run, persistence and stop state. A provider-requested unknown or out-of-order action becomes a
failed auditable Tool result that the next turn can repair.

## D015 — Context is loaded by need

The initial live prompt contains no full Registry, Workflow JSON, image bytes, Run history or
Artifact history. Explicit read tools reveal only the requested bounded subset. Assistant text is
transient conversation context and is not persisted, avoiding hidden chain-of-thought storage.

## D016 — Dry Run evidence is bounded data, not a second vision prompt

The Pipeline Builder receives aggregate image/outcome/warning/model-call/latency/cost fields plus at
most five selected failed or Review summaries. It does not automatically receive image bytes or full
Artifact bodies. A graph revision invalidates the previous validation and Dry Run. Crop verification
is available only after inspecting current evidence, binds a healthy/available Registry
Classification Model, and preserves a user-visible rationale that cites the measured Review rate.

## D017 — Agent grammar is a publish invariant

Pipeline Grammar is not merely an advisory Tool result. Label Pipeline Dry Run and Publish invoke it
inside the Application service, including Decision-before-Commit, bounded uncertainty, model-call,
forbidden-node and Commit-count rules. The Agent may iterate through invalid editable Drafts, but an
invalid graph cannot cross the publish boundary.

## D018 — Agent proposals remain separate and are applied as typed changes

Pipeline Builder output is persisted as its own Suggested Draft. Rust, not React, compares it with
the user's editable Draft and assigns stable change IDs for nodes, parameters, edges, bindings and
policies. Apply selected writes into the existing Draft identity; the Suggested Draft remains audit
evidence. A partial technical selection intentionally drops the optional Label-authoring projection
when it cannot be reconstructed losslessly, while preserving the exact typed DAG for Guided and
Expert rendering. Static validation must still pass before Dry Run or Publish.

## D019 — Progress is a projection of persisted Tool actions

The GUI polls the project-local Agent Session while the request is active. Stage names are derived
from persisted status and the last registered Tool, never from a client timer or hidden model text.
Cancellation targets an Application-owned token for active work and persists terminal state. The
TUI reads the same audit envelope, so GUI and terminal do not maintain competing Agent histories.

## D020 — Undo is a normal Draft mutation, not history rewriting

Apply returns the complete pre-change Draft snapshot. The current GUI offers one-level Undo and
saves that snapshot through the same immutable-status and project checks as manual editing. Undo
does not delete the Agent Session or Suggested Draft and cannot rewrite a Published Version.

## D021 — Domain advice is a declared, bounded Skill resource

An enabled Domain Skill may declare an Advisor resource containing annotation policy and known
risks. Pipeline Builder must load that exact Registry resource before creating a Domain Draft; it
cannot read an arbitrary path. At most four resources and 12,000 characters enter model context,
and their contents are stored as explicit Tool audit rather than hidden prompt material.

## D022 — The default RoboCup recipe uses one ready Detection backend

RoboCup Ball defaults to Image → Detection → Select football → Domain Validators → Decision →
Commit/Review. Model preference resolves only against a matching available or healthy Registry
descriptor. Specialist detection, segmentation, Crop verification and multi-model recovery are
evidence-driven or explicitly configured alternatives; disabled, Unknown or Labs bindings are not
recommendations. Existing immutable specialist workflows remain executable compatibility data.

## D023 — Live credentials are operator state, never task input

The server may prefer the configured workspace VLM only when the user enabled external models and
the local secret store contains a credential. A conversation credential is never copied into that
store. Therefore the M7 external Qwen request is recorded as live-conditional, while the no-key
ScriptedMock demo is labelled as mock evidence and exercises the same Rust Tool boundaries.

## D024 — Provider turns and Tool actions have independent budgets

`maximum_agent_turns` bounds calls back to an LLM Provider. `maximum_tool_calls` bounds registered
actions and persisted audit steps. `AgentSession` records one step per Tool Call, so its step limit
maps to the Tool limit while the live Pipeline Builder loop enforces turns directly. A budget stop
is persisted before every early return; increasing the turn limit is not used to hide a Tool-loop
protocol defect.

## D025 — Release evidence is layered and truth-labelled

The fail-fast acceptance script owns formatting, boundary/secret scans, strict Rust checks, Web
checks, doctor and offline demos. Playwright is a separate full browser gate so its server lifecycle
and screenshots remain diagnosable. ScriptedMock and synthetic results are named as such. External
Qwen and local-weight workers cannot inherit PASS from their protocol adapters or Mock fixtures.
