---
name: annotagent-visual-system
description: Apply or review the AnnotAgent Core and RoboCup AnnotAgent visual system in the repository. Use for logo, branding, design tokens, React GUI theme, Ratatui TUI theme, annotation overlay colors, accessibility, visual QA, or visual-system integration. Do not use it to redesign backend behavior or claim unrelated feature gaps are complete.
---

# AnnotAgent Visual System Skill

When invoked:

1. Locate `design/annotagent-visual-system/`.
2. Read, in order:
   - `README.md`
   - `brand/BRAND-GUIDELINES.md`
   - `docs/UX-GUIDELINES.md`
   - `docs/ACCESSIBILITY.md`
   - `codex/CURRENT-PROJECT-STATE.md`
   - `codex/CODEX-PROMPT.md`
3. Inspect the existing repository before editing. Reuse its component architecture and style pipeline.
4. Treat `tokens/tokens.json`, `tokens/tokens.css`, and `brand/logo/svg/` as canonical. Never sample from `reference/*.png`.
5. Preserve runtime behavior, API contracts, annotation data, keyboard controls, and Git configuration.
6. Implement a narrow, reviewable visual integration. Do not introduce a new UI framework merely to make the screenshots look different.
7. Run the repository's existing formatting, typecheck, test, and build commands. Report only commands actually run.
8. Finish with a diff review focused on accessibility, hard-coded colors, responsive layout, and regressions.
