# AnnotAgent Agent-first UI — execution record

## Current checkpoint — M0 in progress (2026-09-09)

Goal: implement the complete Paper & Graphite Agent-first contract, M0–M5.
This is not a completion declaration. The preceding Conversational Workspace delivery
is the baseline, not evidence that Plan, queue, model selection or continuation meet
this new contract. Previous goal turn produced evidence; this first turn changes the
authoritative acceptance tests and records the new baseline.

### Inputs and protection

- HEAD `b694f06`, branch `main`, initially synchronized with `origin/main`.
- Read the kit README, DESIGN_SPEC, CODEX_PROMPT, tokens JSON/CSS and legacy token map.
  Viewed all six desktop and both mobile PNGs. Opened the self-contained OPEN_ME.html
  through a loopback-only, read-only server on 8798 and exercised Plan → approval
  dialog → execution → interrupted. The prototype explicitly says it jumps straight
  to the stopped state; production must wait for server acknowledgement.
- No applicable AGENTS.md found in this repo or checked ancestors. Unrelated repos'
  instruction files were not applied.
- The design kit is user-provided and currently untracked. Preserve it and the 110
  existing screenshot/output changes. Stage only files authored for this goal.
- User workspace on 8787 is not changed. Tests use generated TEST workspace on 8791
  and the existing fixture transport on 8796. No paid calls, keys, remote changes,
  push, reset/rebase/amend, historical version changes or cleanup.

## Baseline and reusable contracts

| Area | Current evidence | Required change / retained boundary |
| --- | --- | --- |
| Entry/layout | ConversationWorkspace defaults to width 32, separator range 25–50; empty image surface always visible | Conversation navigation + centered thread; on-demand pane defaults 54/46 and can be enlarged. |
| Composer | Two buttons: Save goal and prepare labels / Save message; JSX decides prepare-goal based on current local selection | Unified send intent resolved by server; attachment/reference/mode/model and separate active control. |
| Messages | Ordinary journal items share card styling; specialized Journey forms are inline | Plain thread typography plus compact actual object blocks; reuse refs and persistence. |
| Stop | Server/Application/Storage conversation_stop persists exact targets, signals cancellation, exposes observations | Reuse this machinery; audit safe-boundary acknowledgement and continuation separately, never infer stopped from fetch abort. |
| Planning / consent | conversation_journey and builder services freeze Schema/model/scope/budgets; explicit execute POST dispatches | Add persisted per-turn Plan execution policy, deny vision/formal operations on direct API paths, bind exact approval. Existing authorization alone is not proof of Plan mode. |
| Model | Registry profiles and planner_model_id already exist in scope construction | Persist next-request Agent selection by conversation revision; no rebind of existing visual workflows. |
| Queue | Current message journal persists refs, but not yet proven to represent actionable queued turns | Add minimum queued command semantics with frozen selection and idempotency, no parallel engine. |
| Human answer | Existing Sandbox Revision/outbox/answer delivery and startup recovery | Preserve exact save-before-resume, scope and budget constraints. |
| Tokens | Runtime styles.css imports web/src/annotagent-tokens.css; old visual-system JSON documents itself canonical, CSS is currently duplicated | Migrate existing source and consumption/generation relation rather than append another theme; retain annotation slots independently. |

### Design observations

The eight images put navigation on the left and the thread in the main area; only
human/results states use a split surface. Ordinary assistant text has no enclosing
card. Plan/authorization/human blocks have light hierarchy. The mobile design switches
between conversation and result, not a compressed three-column desktop. Paper/Graphite
is monochrome chrome with quiet blue-gray focus; original pixels remain untouched.
All pictured objects, budgets and model names in the kit are illustrative, not production
bindings or actual model performance. They will not be imported into the Registry.

## Tests and screenshots

- `npm run typecheck && npm test` (web), process 53233 exited 0: 47 files, **232 tests**.
- `ANNOTAGENT_E2E_EVIDENCE_DIR=/tmp/annotagent-agent-first-m0 npm run test:e2e -- conversation-entry.spec.ts agent-first-workspace.spec.ts`,
  process 91330 exited 1: **2 existing entry cases passed, 1 new acceptance case failed**.
  Four expected baseline mismatches: missing Conversations navigation, missing Send,
  two old Save controls, visible empty image panel. This is a real isolated Project
  created through current APIs, not a mocked page or prototype render.
- [Actual empty-project baseline](agent-first-ui/m0-empty-baseline.png), inspected at
  1280×800. It visibly confirms the fixed empty pane and form-like sidebar.
- Failure trace: `web/test-results/agent-first-workspace-Agen-364f1-nd-exposes-one-send-control-chromium/trace.zip`.
  This path is ephemeral; screenshot above is retained as baseline evidence.
- Full Rust and full E2E for this new goal have not yet run. Earlier 714/178 green runs
  belong to the previous goal, not this goal's new requirements.

## Milestones and next work

1. M0: finish direct-API Plan/control failure contracts and current control-state audit.
2. M1: migrate token source, extract thread/composer/navigation/pane using actual data;
   make the new baseline acceptance test pass without weakening its intent.
3. M2: versioned Plan policy and exact approval, including direct API denial tests.
4. M3: stopped-at-safe-boundary receipts, resumable vs terminal controls, queue and
   persisted next-request Registry model selection.
5. M4: preserve human correction/outbox, URLs, reconnect, keyboard and mobile flows.
6. M5: six actual service screenshots, full regression, theme/viewport/200% evidence
   and honest fixture/live/native limitations; update README only with actual app shots.

No milestone is marked complete merely by this baseline or design inspection.

## M0 control-contract audit — unknown completion race

The previous turn was progress (new failing entry acceptance test and baseline commit
7a5ce44). This turn found and fixed an actual Application-layer stop observation bug.

When the stop menu offered multiple active requests, a chosen Provider could become
`in_doubt` before selection. Storage correctly saved `Finished` (nothing left to cancel),
but Application returned `finished` before inspecting the actual remote outcome. This
could hide remote completion/cost uncertainty in the new stopped-state UI.

Added `stop_selection_after_unknown_completion_does_not_hide_remote_uncertainty` with
two real reserved calls in an isolated temporary database. It failed before the fix:
expected `unknown`, got `finished` (process 58348, exit 101). The fix keeps the receipt's
historical selection status but always inspects the selected operation: pending and
unknown observations take priority. It does not cancel the second request, reset its
budget, retry a Provider, or invent a resumable checkpoint.

Verification:

- `cargo fmt --all`: only the two intended Application files changed (excluding existing PNGs).
- `cargo test -p annotagent-application conversation_stop::tests -- --nocapture`:
  3 passed, process 58925 exit 0; includes pending → unknown and exact-owner tests.
- `cargo test -p annotagent-storage conversation_stop -- --nocapture`:
  20 passed, process 17448 exit 0; includes transactional selection, shared siblings,
  cumulative grants, publication fences and late admission.
- `cargo clippy -p annotagent-application --all-targets --all-features -- -D warnings`:
  process 31615 exited 0.

Still open: Plan does not yet have the per-turn policy required here; do not present
existing grant checks as Plan-mode verification. Continuation and queued input remain
work for M2/M3. The M0 entry test remains intentionally red until the actual new layout
and unified server-resolved send contract are connected. No real workspace restarted.
