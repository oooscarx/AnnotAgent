# AnnotAgent Visual System Integration

## Current entry points

- Web: `web/src/main.tsx` imports the single global stylesheet at `web/src/styles.css`. The UI is React + TypeScript + Vite with hand-written components and no Tailwind or third-party component library. The existing stylesheet contains an older forest-green palette, fixed desktop minimum width, and component-level color literals.
- Annotation editor: `web/src/components/AnnotationCanvas.tsx` owns the SVG overlay and geometry interactions. Its current selected/unselected colors are hard-coded and domain-neutral label slots are not represented.
- TUI: `apps/annotagent/src/tui.rs` contains the Ratatui event loop, layout, and the few existing color/style constants in one file. There is no theme abstraction yet.
- Brand surface: `web/index.html` has a title and one theme color, but no favicon, manifest, touch icon, or Open Graph assets. The sidebar uses a text placeholder instead of the formal mark.
- Documentation: the README already describes RoboCup AnnotAgent as AnnotAgent Core plus the RoboCup Skill, but has no visual-system provenance or asset reference.

## Token and asset integration

- Keep `design/annotagent-visual-system/tokens/tokens.json`, `tokens.css`, and `brand/logo/svg/` as the canonical design source.
- Copy the canonical `tokens.css` unchanged to `web/src/annotagent-tokens.css`; import it before component rules from `web/src/styles.css`.
- Rewrite existing component rules against `--aa-*` tokens. Do not create a parallel theme; any temporary compatibility aliases must point back to the canonical variables.
- Put runtime-delivery copies of formal assets in `web/public/brand/`. These copies exist only because Vite must serve them; provenance remains the visual-system package under `design/`.
- Add favicon, touch/PWA icons, a Web manifest, theme color, and Open Graph metadata without introducing font files.

## GUI scope

- Replace the sidebar placeholder with the official RoboCup AnnotAgent dark-surface lockup and use the Core mark at compact breakpoints.
- Restyle Dashboard, Project/image list, run progress, Review, Agent Trace, Settings, Skills, dialogs, empty/loading/error states with the shared tokens.
- Preserve all HTTP calls, event handling, run controls, revision/review behavior, and SVG geometry editing.
- Add accessible button labels, stable focus-visible treatment, explicit disabled states, text-bearing status badges, and responsive layouts for desktop widths and 200% zoom.
- Move annotation presentation to generic `slot1`–`slot8` definitions. Load RoboCup label-to-slot/pattern values from the packaged mapping module; the canvas component must not contain RoboCup category names.
- Keep an annotation list as the keyboard-accessible textual equivalent of the SVG overlay. Selection remains synchronized with the canvas.

## TUI migration

- Add `apps/annotagent/src/tui/theme.rs` with one `AnnotAgentTheme` and `StatusTone` mapping derived from the supplied Ratatui reference.
- Apply the navy base/surfaces, blue selection, teal tool/validator trace, and semantic warning/danger colors without changing commands or the event loop.
- Render `RoboCup AnnotAgent` and `AnnotAgent Core · RoboCup Skill` in the header. All status values remain visible as text.
- Use a compact layout below the normal terminal width/height so saturating calculations remain safe and small terminals do not panic.

## Functional boundary kept unchanged

This integration does not add batch checkpoints, multi-image HTTP execution, annotation import, provider fallback, a new front-end framework, runtime behavior, or missing geometry-authoring features. The existing documented limitations remain limitations.

## Verification

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
npm --prefix web run typecheck
npm --prefix web test -- --run
npm --prefix web run build
```

Browser smoke tests cover favicon/title and every main page at 1280×720, 1440×900, and 1920×1080, plus a 200% zoom-equivalent viewport, keyboard focus, reduced-motion CSS, overlay/annotation-list parity, and pause/cancel controls. TUI smoke tests cover normal and constrained terminal sizes while preserving run, pause/resume, cancel, GUI, and quit keys.

## Verification record

- Canonical package checksums passed for all 104 source files, and the runtime token CSS is byte-for-byte identical to the canonical `tokens.css`.
- Rust formatting and Clippy passed; the full workspace test suite passed with 46 tests; the all-feature workspace build passed.
- Web typecheck, two Vitest tests, and the Vite production build passed. Asset dimensions and manifest JSON were validated.
- Browser smoke testing used an isolated workspace and a deterministic review run. Dashboard, Project, run progress, Review, annotation list/overlay, Validator evidence, Agent Trace, Settings, and Skills rendered with no console errors or horizontal overflow at 1280×720, 1440×900, 1920×1080, and a 720×450 200%-zoom-equivalent CSS viewport. Exact browser zoom emulation was unavailable in the test surface.
- Native buttons/links/inputs, accessible names, selected-state text, and visible focus outlines were observed. Automated sequential Tab traversal was not reliable in the browser-control surface, so a final manual keyboard pass remains recommended.
- The TUI rendered successfully in a real 80×24 truecolor PTY, displayed both product-title lines and text statuses, accepted `q`, left the alternate screen, and restored the cursor. Test backends at 120×32, 80×20, and 40×8 also rendered without panic; ANSI-256 mapping is covered by the same Theme abstraction.
