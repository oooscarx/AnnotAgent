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
