# Visual finish — first-version Paper & Graphite

## Baseline and merge safety (2026-09-09)

Fetched origin. Local/remote main both `c41b281b49252d520117029d39611865133798af`; local/remote integration both `3e9d8f51d89be21851c4920ddeda4ea06d2c7a43`. Main is an ancestor of integration. **Main was NOT merged:** its existing worktree contains unrelated modified/untracked screenshots and design files. No stash, ref movement, checkout, data cleanup or overwrite was performed. No backup ref needed yet because no main merge was attempted. Once that worktree is safely clean, create a unique backup branch and normal fast-forward; do not bypass this block with update-ref.

Visual worktree `/Users/oscar/Documents/my_workspace/AnnotAgent-visual-finish`, branch `codex/ui-visual-finish`, created directly from integration. Main/backend/frontend/integration source worktrees retained. No push. No Rust/API/credential/Published Version changes. 8787 and approved 5174 remain untouched; HTTP visual verification is on isolated 8810/8811, marked temp workspace `TEST-agent-ui-hbjdbkod`.

Read both visual-finish instructions, first-version design spec, prototype CSS/HTML and five supplied reference PNGs. Read current production entry, integration limitations and front/backend handoffs. No AGENTS.md found in the applicable ancestry or repository. Original design assets are untracked in main, so production uses only copied first-version sprite and monochrome marks, not a copied prototype or execution engine.

## First visible change

- One typed Icon/IconButton implementation, original 29-symbol sprite, 18/16px currentColor SVG. Replaced glyph chrome in App, Composer, Settings, Plan and canvas. Explicit dark/light monochrome marks; no invert filter.
- 216px sidebar with real chevron/folder slots, task dot and single-line ellipsis/full title; grouped spacing and independent scrolling. Composer controls use SVG, compact mode/model/send; IME/idempotency handlers unchanged.
- One-pixel visible separator with 9px pointer hit region and original keyboard/drag behavior. Project menu anchored to trigger, clamped to viewport, Escape/outside close. Model popover placement clamped without changing selection protocol.
- Existing task evidence chooses a primary action; secondary actions remain under one expandable row. An exact HTTP approval hides duplicate entry points; no simulated awaiting-approval button in HTTP mode. Authorizations retain their exact payloads and confirmation.
- Six Settings sections use shared control columns and responsive rows; existing editor save/cancel, CAS and dirty guard retained.
- Canvas shares labelColor's stable mapping rather than bottle-specific colors. Box, text and handles agree; label/handle screen scale adjusts to resize/zoom. Original pixels, pointer transforms, bbox coordinates and normalized save logic untouched.
- Fixture-only IconGallery added; TEST markers stay visible on narrow HTTP layouts. No new backend capability promised.

## Evidence and current checks

Before: actual unchanged 8804 captured under `web/public/evidence/agent-ui-integration/3e9d8f51d8-1788957154496`. Initial after under `3e9d8f51d8-1788957555074` records working source patch; this revealed the desktop mobile-menu CSS collision, since fixed. Final committed-source capture and reference/before/after gallery follow.

First unit run: 264 tests passed / 54 files; typecheck/build passed. Preview initially 18/20: two old glyph-based selectors (task punctuation / model suffix), corrected without weakening dirty guard or model focus assertions. First HTTP 12/16: three obsolete Settings/footer glyph selectors and one blank-page resource race because a build changed dist during the running suite. Corrected selectors; subsequent suites freeze dist until completion. Do not call this initial run a pass.

## Preserved limitations

No new missing-box creation or candidate-reference Agent replanning in HTTP. Inline canvas is sample/human evidence; formal batch results use existing management. No automatic queue dispatcher, generic resume, invented system-wide cost or model-install success. Real-model precision and human usability are not claimed. Native browser 200% zoom still requires separate verification; CSS/viewport scaling must be labeled separately. Final regression, screenshots and build hash evidence will be recorded below.

## Final visual baseline and verification

Application source: `9c84582390d1424e46f787d1a7748f39135d5851`. Implementation commits: `593a4a6` (visible UI and actual screenshots), `29e0183` (icon/layout regressions), `9c84582` (Settings alignment and canvas halo). Final evidence-only commit follows these; screenshot manifests deliberately retain the application-source SHA.

Actual comparison: http://127.0.0.1:8812/evidence/visual-finish/index.html . Final HTTP screenshots: `web/public/evidence/agent-ui-integration/9c84582390-1788958415585`; Fixture screenshots and original reference copies: `web/public/evidence/visual-finish`. Each final screenshot records source, URL, viewport, DPR, theme and TEST/Fixture boundary. Reference files have SHA256 provenance and are never counted as application evidence. `build-proof.json` verifies served HTML, bundled assets and original SVG assets against the local production build byte-for-byte.

Final isolated service: 8812 / TEST model 8813, `/private/var/folders/fk/x_vdk3nd7ws51fx8fzxwn6mm0000gn/T/TEST-agent-ui-ww_t5rsy`; production HttpAdapter, real HTTP routes and SQLite, external explicit test model. Preview uses 5178 and only Fixture Adapter. Neither accesses the user's workspace or paid Providers.

Checks actually run:

- Web typecheck, production build and Preview build passed.
- Unit: 267 passed in 55 files.
- Preview/browser: 27 passed, including original IME/send/pane/dirty-guard assertions and added icon, popup, sidebar and canvas scaling checks.
- Fresh isolated real HTTP suite: 16/16 passed. After final styling: five targeted HTTP cases passed (owned thread, six Settings, Plan authorization, Sample authorization, stop/unknown), then screenshots recaptured.
- Layout checked at 1440, 1280, 1024 and 390px widths. IconGallery includes 16/18px, light/dark, hover, focus, disabled and explicitly labeled CSS 200% captures. Browser-native zoom shortcut attempt did not change measured viewport/DPR: native 200% remains **unverified**, not passed.
- Rust unchanged: no new full Rust run this turn; prior integration regression is not represented as a new run. Full legacy browser suite, native OS IME, live model precision and real novice usability were not executed.

Management/history/recycle-bin capabilities remain through the project management menu and existing unique management pages; no Runs/Review/Pipeline global navigation restored. Original Plan payloads, CAS, explicit queue authorization, stop receipts, supported Batch resume, human-save ownership and normalized geometry remain unchanged. Fixture continues to demonstrate capabilities not generally supported by HTTP; there is no HTTP-to-Fixture success fallback.

Remaining differences: real server receipts and permission scopes are denser than reference prose; actual TEST imagery is not the prototype illustration; management pages remain separate existing pages. Long canvas labels still use a bounded text preview, stable label hashing may reuse colors, and these are not claims of perfect visual equivalence. Unsupported missing-box/replanning/automatic-dispatch capabilities were not added.

Main remains `c41b281b49252d520117029d39611865133798af`, **not merged** because the original worktree still contains user modifications. Integration remains `3e9d8f5`. No push, remote changes, reset, rebase, amend, real-data cleanup, Published Version changes or 8787 restart. Main fast-forward requires the user's existing files to be safely resolved first. The local dependency symlink `web/node_modules` is not part of the deliverable.
