# AnnotAgent Visual System Integration

## Product hierarchy

AnnotAgent Core owns the global product identity: app mark, lockup, favicon/PWA/OG assets, tokens, common icons, semantic statuses, and eight generic annotation slots. Skill extensions may add a badge, icon, example lockup, and label visual profile. Example Project assets remain contextual.

Runtime delivery follows this boundary:

```text
web/public/brand/
├── core/
│   ├── annotagent-*.svg
│   ├── favicon and PWA/OG assets
│   └── icons/
└── skills/
    └── robocup/
        ├── skill-badge.svg
        └── example application lockups
```

Canonical sources and provenance remain under `design/annotagent-visual-system/`; `web/public` contains Vite delivery copies.

## Web integration

`web/src/styles.css` imports the canonical token copy in `web/src/annotagent-tokens.css`. The shell uses the Core lockup and five task destinations: Home, Projects, Runs, Review, and Settings. Automation belongs to Project Build; Models and Capabilities belong to Settings. No-Project Home and empty states are domain-neutral.

`AnnotationCanvas.tsx` renders geometry and accepts an `AnnotationVisualContext`. It contains no Skill label names. `annotationVisuals.ts` resolves a label through:

1. Project explicit override;
2. Skill visual mappings, with Skill conflicts sorted by stable Skill id;
3. schema visual mapping;
4. stable label hash fallback.

The RoboCup mapping is isolated in `web/src/skills/robocup/visualProfile.ts` and reads the packaged canonical label map.

## TUI integration

`apps/annotagent/src/tui/theme.rs` defines one domain-neutral truecolor/ANSI-256 theme. The title is **AnnotAgent** with **Composable Annotation Workflow Runtime**. With no Project, it says `No project opened · Use /open or /init`. Project, Workflow, and Skill names are loaded dynamically after `--project` or `/open`.

The event loop, run/control shortcuts, textual status labels, and constrained-terminal layout remain intact.

## Accessibility and behavior

Color never replaces a label, pattern, geometry, or status word. Buttons retain accessible names, disabled actions explain why they are unavailable, and `:focus-visible` uses the shared high-contrast ring. Review and Run canvases expose a keyboard-operable structured annotation list equivalent to the SVG overlay. Motion collapses to 0.01 ms under `prefers-reduced-motion`.

Release layouts use at most one solid primary button per page state, no nested `.panel` containers, and no more than three equal first-screen outcome metrics. Project Header keeps three numeric facts at equal weight and moves Automation, Active Run, and readiness into a secondary status line. The same surfaces are browser-covered at 1024 px and 720×450 without horizontal overflow.

## Historical migration note

Visual System 1.0 originally integrated the RoboCup example lockup into the global shell. The product-hierarchy migration retained those source assets but moved runtime use to the Skill namespace. References to the old lockup in source-package previews or migration records are historical/example assets, not current branding.

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

Acceptance additionally checks a domain-neutral no-Project Web/TUI state, Project-scoped Skill/Workflow context, deterministic generic fallback visuals, Skill visual-profile mapping, and production asset paths.
