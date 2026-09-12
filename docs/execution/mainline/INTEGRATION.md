# Mainline Integration

Only Frontend 1 changes this branch and file.

| Component | Fixed input | Integrated commit | Status |
|---|---|---|---|
| Common baseline | `d3220cb` | `d3220cb` | fixed |
| Frontend 1 seam | VisualSelection / task read model / F2-F3 slots | `7fd1439` | integrated |
| Frontend 1 intake/messages | missing-only intake and persisted Agent reply projection | `2c4b032` | integrated |
| Frontend 1 candidate send | frozen SampleCandidate conversion and stale-Schema guard | `15a4ece` | integrated |
| Frontend 2 | Sample/Formal review, package UI, ReviewWorkItem seam | `43e416b`, `4cb6c14`, `309b7ed`, `238a838` | integrated; HTTP B2/B3 pending |
| Frontend 3 | Capability setup / guarded Settings return | `2c52266` (source `943a6ab`) | integrated; task mount awaits capability read model |
| Backend | B0 contract only; planned fields not called | `5a732c5` (source `99d16f9`) | contract integrated |
| Backend B1 | passive Mainline projection + authorized local advance | `0f62007` (source `22ddee9`) | integrated and wired |
| Frontend 2 seam | delivery VisualSelection and lineage service slots | `b1556d7` (source `b6934bf`) | integrated; component delivery pending |

The earlier context-history branch and backend archive delivery remain separate committed work. They are not silently copied into this branch and will be integrated only by fixed commit after compatibility verification.
