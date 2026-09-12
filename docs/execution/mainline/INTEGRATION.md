# Mainline Integration

Only Frontend 1 changes this branch and file.

| Component | Fixed input | Integrated commit | Status |
|---|---|---|---|
| Common baseline | `d3220cb` | `d3220cb` | fixed |
| Frontend 1 seam | VisualSelection / task read model / F2-F3 slots | `7fd1439` | integrated |
| Frontend 1 intake/messages | missing-only intake and persisted Agent reply projection | `2c4b032` | integrated |
| Frontend 1 candidate send | frozen SampleCandidate conversion and stale-Schema guard | pending commit | integrated locally |
| Frontend 2 | ReviewWorkItem / package | pending | not integrated |
| Frontend 3 | Capability setup / task return | pending | not integrated |
| Backend | B0 contract only; planned fields not called | `5a732c5` (source `99d16f9`) | contract integrated |

The earlier context-history branch and backend archive delivery remain separate committed work. They are not silently copied into this branch and will be integrated only by fixed commit after compatibility verification.
