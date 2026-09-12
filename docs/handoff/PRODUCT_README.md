# Product README handoff

## Delivered

- Chinese README.md remains the main entry; README.en.md links back to it.
- Small light/dark monochrome banners, six-block responsibility diagrams with narrow-screen variants, one full real historical workspace capture, and separately labeled TEST review/package captures.
- Editable SVG and generation scripts, CLAIMS.md, ASSET_MANIFEST.json, GUIDE.md, EVIDENCE.md and VALIDATION.md.
- Local approximate Markdown previews for light/dark, desktop/390 px. README uses only Markdown and supported picture/source/img HTML; preview CSS is not a product or GitHub dependency.

## Source alignment

Documentation branch: `codex/product-readme`, created from `eaeae4cf8ee92bded9afb5577d2c9b70bc112de0`. That history branch is NOT the deployment target and must not be merged wholesale into main for this documentation change.

Product facts were checked against delivery baseline `d3220cb` and mainline snapshot `1b1d9de16d5181e249967d4e8e47b977a4b645d3`. Backend subsequently supplied B3 `1e2bac549f52ed98c2f9fbec9ed9f288f3e05251`, observed integrated as `a7053b5`; final frontend acceptance remains pending. The README conservatively keeps manual confirmation wording until final verification.

Frontend 1 owns integration. Apply only the document commit to the confirmed integration branch after checking wording and links. No push, merge, main update, remote change, reset, rebase or amend was performed by this task.

Documentation commit: the commit containing this handoff (resolve with `git log -1 --format=%H -- docs/handoff/PRODUCT_README.md`). The user-facing delivery includes its literal SHA; a commit cannot contain its own final hash.

## Remaining evidence / release gates

1. Frontend 1 has not yet supplied a final public application SHA or matching genuine-data screenshot endpoint. The existing 8788 capture is explicitly historical, with unknown static UI build SHA; it must not be called the final integrated UI.
2. Real B-Human evidence covers saved model samples. The formal correction and ready ZIP evidence is marked TEST. A same-task real model → reviewed formal results → downloaded package demonstration has not been established here. No synthetic replacement was made.
3. The TEST review screenshot does not show a saved-edit receipt. Source test records report a persisted one-pixel correction, but that record is not a screenshot of a real human correction. A full real correction/saved-state capture remains pending.
4. Complete package scope is Ultralytics YOLO detection v1 only. Actual existing TEST ZIP hash/CRC/file list was checked in this task, but no real framework loader or training was run.
5. Root LICENSE text is missing despite Cargo MIT metadata. Maintainer must resolve license text; no license was invented. Dataset screenshot redistribution rights require source-owner/maintainer verification before public release.
6. UIAPI-018 exists on the separate history baseline but is not assumed integrated. Do not restore its feature claim merely because it is in the documentation worktree.

## Validation and non-changes

Normal npm ci and Web production build passed on the isolated worktree; no dependency manifests/locks changed. Rust source start was inspected but not executed. All local README links and asset hashes pass; default loaded static images approximately 0.58 MB. Local previews have no overflow or broken images; not tested on the real GitHub website.

Only allowed documentation paths were changed. Existing runtime brand files, source, databases, real workspace, engineering evidence directories, CI and deployment settings were read-only. No Provider calls or model-weight downloads occurred. No original dataset/ZIP/database/full chat export was added to Git.

## Requested engineer evidence

User-authorized questions were sent to Frontend 1, Frontend 2, Frontend 3 and Backend with exact required evidence. Backend supplied ML-012 source/semantics. Frontend 3's existing model-preparation handoff was inspected (943a6ab lineage). F1 final source alignment and F2 additional real captures remain pending; missing replies are not approval.
