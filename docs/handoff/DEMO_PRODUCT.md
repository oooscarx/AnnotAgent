# Demo product handoff

## Delivery

Product branch: `codex/demo-product-assets`<br>
Starting application SHA: `8a7d8007904a00af62f3b179c216b8a3b2732a1d`<br>
Backend contract source read: `0c6ceb6623848241ac2b82dd67cbddd36b43673a`<br>
Pack identity: `object-detection-review@1.0.0`

The requested filename `ANNOTAGENT_DEMO_PRODUCT_PROMPT.md` was not present in the
available worktrees. The role instructions were present in
`design/annotagent-guided-demo-r1-r6/06_PRODUCT_MANAGER.md`, with the shared contract
in `01_SHARED_CONTRACT.md`; both were read and used as the governing source.

This role changed only `README.md`, `README.en.md`, `docs/product/demo/**`,
`docs/handoff/DEMO_PRODUCT.md`, and `examples/demo-packs/**`. No `web/`, `crates/`,
database, workspace, dependency, CI, release, or runtime brand files were changed.
No remote model or paid image-generation service was called.

The product commit is the commit containing this handoff. Frontend 1 owns integration
into `codex/demo-onboarding-integration`; do not merge this branch into main or push it
as part of this handoff.

## Actual pack contents

`examples/demo-packs/object-detection-review/1.0.0/` contains:

- six 1024×768 PNG originals in `images/`;
- six 384×288 PNG previews in `thumbnails/`;
- one 640×360 unannotated contact-sheet thumbnail;
- one aggregated Backend-contract candidate document in `references/candidates.json`;
- `manifest.json`, `ATTRIBUTION.md`, a pack README and reproducible generator;
- generated `asset-checks.json` for editorial review. Runtime identity comes from the
  manifest and Backend validator, not from this editorial check file.

All visual inputs are original procedural **synthetic** illustrations. They contain no
third-party photos, logos, people, private data, credentials or external downloads.
Each PNG records synthetic provenance, pack identity and `CC0-1.0` in metadata.
`ATTRIBUTION.md` applies CC0 to the generated images, thumbnails and reference JSON,
links the official deed/legal code, and separates the generator's software license.

The pack is under 1 MiB, below the 12 MiB target. Image hashes, byte counts, dimensions,
MIME types, fixed split and source group are carried by the manifest. Four train images
and two validation images are independent scenes; both splits include cup and bottle.

## Coverage

| Image | Split | Content | Preset candidates | Purpose |
| --- | --- | --- | --- | --- |
| image-01 | train | one large cup | 1 cup | clear large target and handle extent |
| image-02 | train | one large bottle | 1 bottle | clear large target |
| image-03 | train | cup + two bottles | 3 objects | multi-target and scale variation |
| image-04 | train | books + plant | none | explicit negative; never auto-confirmed |
| image-05 | validation | cup + bottle | 2 objects | both labels represented in validation |
| image-06 | validation | cup + bottle + strong shadow | 2 objects | cup candidate intentionally includes shadow and nearby bottle area |

`image-06`'s loose cup candidate is visible in
`docs/product/demo/reference-candidates.png`. The sheet says synthetic, preset and no
model inference. It is not a UI capture. The original and preview images have no boxes.

## Candidate boundary

Candidates conform to the Backend `annotagent.demo-candidates` v1 shape, use normalized
`x/y/width/height`, stable label IDs and source artifact IDs, and bind exact image SHA.
Every candidate score is `null` under the Backend contract: these supplier references
must not appear as model confidence.

The candidate asset is only for the named preset mode and offline evaluation. It must
never enter live-model prompts, requests, tool input or caches. Preset import is not a
model Run and does not create token/fee rows. Supplier review is historical provenance,
not the current user's acceptance. `image-04` still needs an explicit negative decision;
all other images need current-snapshot review before package admission.

## Approved wording

Primary entry:

> 标注桌面物品<br>
> 6 张图片 · cup / bottle · YOLO 检测数据包<br>
> 素材类型：原创合成图片

Actions:

- `用我的模型试跑`
- `免配置体验：预置候选`

Preset explanation:

> 使用随示例提供的候选，不发生本次模型推理。可以检查、修改，并在逐图
> 审核完成后生成数据包。

Selected example goal:

> 请标出这 6 张图片里的杯子和瓶子，准备 Ultralytics YOLO 检测数据。
> 边界不确定时让我确认，审核齐全后打包。

Detailed start, progress, review and completion text is in
`docs/product/demo/ONBOARDING_COPY.md`. Do not convert this selected example goal into
a forged user-authored history message.

## README change

Chinese and English README files add a short example section with the real unannotated
pack contact sheet, asset/license link and explicit difference between live-model and
preset modes. They do not state that the integrated Demo feature is already available.
They say the final app screenshot is still pending.

## Final screenshot request

After Frontend 1 integrates the exact Backend and frontend commits, capture the formal
app at the integrated SHA. Use a fresh Demo Project and this pack. Record beside each
image:

- application and Web asset SHA;
- URL, capture time, viewport/DPR and theme;
- `object-detection-review@1.0.0` plus pack digest;
- live-model or preset mode;
- Provider/model and whether a real request occurred, or “preset candidates; no model request”;
- crop/redaction; expected default is a full application frame with none;
- what the frame proves and cannot prove.

Required frames are the example entry/start disclosure, one preset review showing the
clear provenance label and `image-06` boundary problem, and one actual Ready/download
receipt after six current-user decisions. Do not reuse a TEST screenshot, older B-Human
session, concept diagram, or modify the DOM/database to make a state appear.

## Open integration points

1. Backend is revising the frozen contract so image assets carry exact `split` and
   `source_group`, and the pack carries a machine-readable CC0 license bound to the
   attribution asset. This handoff follows the fields supplied in DEMO-PM-007/008;
   integration must use the Backend commit containing those revisions.
2. Run the final Backend validator after its implementation lands. Local JSON parse,
   image decode, hash and geometry checks do not replace the Rust catalog validator.
3. The repository still needs its own root license resolution. The pack's CC0 notice
   is complete for pack assets but does not fix the application-level licensing issue.
4. The synthetic pack tests workflow mechanics only. It is not model-accuracy evidence
   and is not sufficient training data.
