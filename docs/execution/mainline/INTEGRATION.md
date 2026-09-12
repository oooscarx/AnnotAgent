# Mainline Integration

Only Frontend 1 changes this branch and file.

| Component | Fixed input | Integrated commit | Status |
|---|---|---|---|
| Common baseline | `d3220cb` | `d3220cb` | fixed |
| Frontend 1 seam | VisualSelection / task read model / F2-F3 slots | `7fd1439` | integrated |
| Frontend 1 intake/messages | missing-only intake and persisted Agent reply projection | `2c4b032` | integrated |
| Frontend 1 candidate send | frozen SampleCandidate conversion and stale-Schema guard | `15a4ece` | integrated |
| Frontend 2 | Sample/Formal review, package UI, ReviewWorkItem seam | `43e416b`, `4cb6c14`, `309b7ed`, `238a838`, `3279929` | integrated and wired to canonical B2/B3 reads |
| Frontend 3 | Capability setup / guarded Settings return | `2c52266`, `dbf44b3`, `76507bc` | integrated; task setup/recheck mounted in `361c402` |
| Backend | B0 HTTP contract and final evidence | `5a732c5`, `d40dc21` | contract/evidence integrated |
| Backend B1 | passive Mainline projection + authorized local advance | `0f62007` (source `22ddee9`) | integrated and wired |
| Frontend 2 seam | delivery VisualSelection and lineage service slots | `b1556d7` (source `b6934bf`) | integrated |
| Backend B2 | canonical task Sample selections | `0e776cd` | integrated and consumed by `62ab3f6` / `0550b5e` |
| Backend B3 | processing scope, formal lineage, review/package admission | `a7053b5`, `c5d741d`, `dcf4abc` | integrated; production UI wired by `d352706` / `2401e27` |
| Backend B4 | passive capability readiness and Task cost | `5a827d5` | integrated; no probe/install/write on read |
| Frontend 1 F1-2 | formal Composer reference, exact processing preview, task model setup | `2401e27`, `361c402` | integrated; full Web suite/build passed |
| Backend ML-020 | exact saved-Journey Sample continuation action | `28e62b2` (source `e705e63`) | integrated; Frontend exact read/approval wiring complete |
| Task history UI | multi-turn/task trace and context archive controls | `44ef0c7`–`9437bc5` | integrated; real read-only history verified |
| Backend UIAPI-018 | inert versioned context archive/import | `474cad6` (source `eaeae4c`) | integrated; browser round trip completed by ML-022 |
| Backend ML-021 | executable Registry VLM Sample and failed-admission projection | `cf85c0b` (source `e9626ae`) | integrated; fresh bounded Journey Sample succeeds |
| Frontend 1 truthful Journey UI | combined fresh Plan/Sample scope plus saved-Draft recovery | `f871952` | integrated; exact HTTP/browser regression passes |
| Backend ML-022 | browser-stable context archive hash | `4e2191d` (source `9f91faa`) | integrated; save/load and lost-response browser round trip pass |

The context-history frontend series and backend archive delivery were integrated only by their fixed commits after compatibility review; their source branches remain separate and unchanged.

G3 structural delivery evidence is complete: an isolated real-HTTP TEST-provider run crossed formal review and explicit package authorization, produced a downloadable ZIP with SHA-256 `a2a0242282c3e2d112ba1b11009dcfe161e0a175631ac858c1606227adc6b89d`, and passed independent pairing/row/hash checks. Real Provider accuracy remains outside this engineering acceptance and has not been invoked.

ML-021 and ML-022 are resolved on this branch. Fresh Plan/Sample work executes only inside its one explicitly displayed bounded Journey authorization; formal processing remains a separate confirmation. A persisted failed admission is blocked instead of re-offered. Task-history/context-archive is compatibility-audited and integrated: task trace reads remain passive, browser JSON round trips retain integrity, and loaded archives remain inert evidence with no grants or dispatch.

Latest isolated TEST evidence passed 8/8 relevant browser cases, 112 Web files / 376 unit tests, Web typecheck/build, all library tests for Application/Server/Storage, strict Clippy for those crates and fmt. No main merge, push, real workspace write or paid/Live Provider call occurred.
