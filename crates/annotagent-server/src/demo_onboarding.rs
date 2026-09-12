//! Repository allowlisted Demo catalog and idempotent onboarding commands.

use super::{ApiError, ApiResult, ServerState};
use annotagent_application::stable_project_id;
use annotagent_core::{
    Annotation, AnnotationId, AnnotationProvenance, AnnotationSource, AnnotationValue, ArtifactId,
    ImageId, InputModality, ModelCapability, ModelProfileId, ModelProfileStatus, NormalizedRect,
    ReviewStatus, TaskId, TaskKind,
    dataset_delivery::{
        DETECTION_PROFILE, DETECTION_PROFILE_REVISION, DatasetSplit, DeliveryImage, DeliveryLabel,
        DeliveryReviewPolicy, DeliverySplitPolicy, TaskDeliveryIntent, TrainingTarget,
    },
};
use annotagent_storage::{
    DemoImageSeed, DemoPresetAnnotationSeed, DemoSourceMode, DemoSourceProvenance,
    DemoStartReceipt, DemoStartScope, DemoStartSeed, DemoStartStatus, StorageError,
};
use axum::{
    Json,
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderValue, Response, StatusCode, header},
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

const CONTRACT_VERSION: &str = "demo-catalog-v1";
const START_CONTRACT_VERSION: &str = "demo-start-v1";
const ALLOWLIST: &[(&str, &str)] = &[("object-detection-review", "1.0.0")];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestDisplay {
    title: String,
    summary: String,
    learning_objectives: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestProject {
    display_name: String,
    goal: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestAsset {
    id: String,
    kind: String,
    path: String,
    sha256: String,
    mime_type: String,
    bytes: u64,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(default)]
    split: Option<String>,
    #[serde(default)]
    source_group: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestLabel {
    id: String,
    name: String,
    annotation_kind: String,
    color: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestTrainingTarget {
    framework: String,
    task: String,
    format_revision: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestSplitPolicy {
    train: f64,
    validation: f64,
    test: f64,
    seed: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestTask {
    source_image_asset_ids: Vec<String>,
    labels: Vec<ManifestLabel>,
    training_target: ManifestTrainingTarget,
    split_policy: ManifestSplitPolicy,
    review_policy: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetMode {
    enabled: bool,
    asset_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LiveMode {
    enabled: bool,
    required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestModes {
    preset_candidates: PresetMode,
    live_model: LiveMode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestLicense {
    spdx_id: String,
    source_url: String,
    attribution_asset_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryManifest {
    schema_version: u32,
    demo_id: String,
    version: String,
    display: ManifestDisplay,
    project: ManifestProject,
    assets: Vec<ManifestAsset>,
    task: ManifestTask,
    modes: ManifestModes,
    license: ManifestLicense,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetCandidatesFile {
    format: String,
    version: u32,
    demo_id: String,
    demo_version: String,
    images: Vec<PresetCandidateImage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetCandidateImage {
    image_asset_id: String,
    image_sha256: String,
    candidates: Vec<PresetCandidate>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetCandidate {
    id: String,
    label_id: String,
    annotation_kind: String,
    value: PresetBox,
    score: Option<f64>,
    source_artifact_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone)]
struct LoadedManifest {
    manifest: RepositoryManifest,
    root: PathBuf,
    manifest_sha256: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DemoCatalogPage {
    contract_version: &'static str,
    catalog_revision: String,
    items: Vec<DemoCatalogEntry>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
struct DemoCatalogEntry {
    demo_id: String,
    version: String,
    manifest_sha256: String,
    title: String,
    summary: String,
    learning_objectives: Vec<String>,
    image_count: usize,
    labels: Vec<String>,
    delivery_format: String,
    thumbnail_asset_id: String,
    thumbnail_url: String,
    license: ManifestLicense,
    modes: Vec<DemoModeAvailability>,
}

#[derive(Debug, Serialize)]
struct DemoModeAvailability {
    source_mode: DemoSourceMode,
    status: &'static str,
    reason: Option<&'static str>,
    required_capabilities: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PublicManifest {
    contract_version: &'static str,
    catalog_revision: String,
    manifest_sha256: String,
    schema_version: u32,
    demo_id: String,
    version: String,
    display: ManifestDisplayPublic,
    project: ManifestProjectPublic,
    assets: Vec<PublicAsset>,
    task: PublicTask,
    modes: PublicModes,
    license: ManifestLicense,
}

#[derive(Debug, Serialize)]
struct ManifestDisplayPublic {
    title: String,
    summary: String,
    learning_objectives: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ManifestProjectPublic {
    display_name: String,
    goal: String,
}

#[derive(Debug, Serialize)]
struct PublicAsset {
    id: String,
    kind: String,
    sha256: String,
    mime_type: String,
    bytes: u64,
    width: Option<u32>,
    height: Option<u32>,
    split: Option<String>,
    source_group: Option<String>,
    download_url: String,
}

#[derive(Debug, Serialize)]
struct PublicTask {
    source_image_asset_ids: Vec<String>,
    labels: Vec<ManifestLabel>,
    training_target: ManifestTrainingTarget,
    split_policy: ManifestSplitPolicy,
    review_policy: String,
}

#[derive(Debug, Serialize)]
struct PublicModes {
    preset_candidates: PublicPresetMode,
    live_model: PublicLiveMode,
}

#[derive(Debug, Serialize)]
struct PublicPresetMode {
    enabled: bool,
    asset_id: String,
}

#[derive(Debug, Serialize)]
struct PublicLiveMode {
    enabled: bool,
    required_capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StartDemoRequest {
    command_id: Uuid,
    demo_id: String,
    demo_version: String,
    source_mode: DemoSourceMode,
    model_profile_id: Option<ModelProfileId>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

fn catalog_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo-packs")
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn check_path(path: &str) -> anyhow::Result<()> {
    let value = Path::new(path);
    if value.is_absolute()
        || value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        anyhow::bail!("Demo asset path is not a normal relative path")
    }
    Ok(())
}

fn reject_symlink_components(root: &Path, relative: &str) -> anyhow::Result<()> {
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            anyhow::bail!("Demo asset path contains a non-normal component")
        };
        current.push(component);
        if std::fs::symlink_metadata(&current)?
            .file_type()
            .is_symlink()
        {
            anyhow::bail!("Demo asset path cannot contain a symlink")
        }
    }
    Ok(())
}

fn expected_mime(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|value| value.to_str()) {
        Some("png") => Some("image/png"),
        Some("json") => Some("application/json"),
        Some("md") => Some("text/markdown"),
        _ => None,
    }
}

fn load_manifest_from(root: &Path, demo_id: &str, version: &str) -> anyhow::Result<LoadedManifest> {
    if !ALLOWLIST.contains(&(demo_id, version)) || !valid_id(demo_id) {
        anyhow::bail!("Demo or version is not allowlisted")
    }
    let parsed_version = Version::parse(version)?;
    if parsed_version.to_string() != version {
        anyhow::bail!("Demo version must be an exact canonical SemVer")
    }
    let pack = root.join(demo_id).join(version);
    for component in [root.to_path_buf(), root.join(demo_id), pack.clone()] {
        if std::fs::symlink_metadata(&component)?
            .file_type()
            .is_symlink()
        {
            anyhow::bail!("Demo catalog cannot contain symlinks")
        }
    }
    let pack = pack.canonicalize()?;
    let root = root.canonicalize()?;
    if !pack.starts_with(&root) {
        anyhow::bail!("Demo pack escaped its catalog root")
    }
    let manifest_path = pack.join("manifest.json");
    if std::fs::symlink_metadata(&manifest_path)?
        .file_type()
        .is_symlink()
    {
        anyhow::bail!("Demo manifest cannot be a symlink")
    }
    let bytes = std::fs::read(&manifest_path)?;
    let manifest_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let manifest: RepositoryManifest = serde_json::from_slice(&bytes)?;
    if manifest.schema_version != 1
        || manifest.demo_id != demo_id
        || manifest.version != version
        || manifest.display.title.trim().is_empty()
        || manifest.project.goal.trim().is_empty()
        || manifest.task.review_policy != "human_whole_image"
        || manifest.task.training_target.task != DETECTION_PROFILE
        || manifest.task.training_target.format_revision != DETECTION_PROFILE_REVISION
    {
        anyhow::bail!("Demo manifest identity or task contract is invalid")
    }
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for asset in &manifest.assets {
        if !valid_id(&asset.id)
            || !ids.insert(&asset.id)
            || !paths.insert(&asset.path)
            || !hex_digest(&asset.sha256)
        {
            anyhow::bail!("Demo assets require unique valid IDs, paths and lowercase SHA-256")
        }
        check_path(&asset.path)?;
        reject_symlink_components(&pack, &asset.path)?;
        let path = pack.join(&asset.path);
        if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            anyhow::bail!("Demo assets cannot be symlinks")
        }
        let canonical = path.canonicalize()?;
        if !canonical.starts_with(&pack) || !canonical.is_file() {
            anyhow::bail!("Demo asset escaped its pack")
        }
        let payload = std::fs::read(&canonical)?;
        if payload.len() as u64 != asset.bytes
            || format!("{:x}", Sha256::digest(&payload)) != asset.sha256
            || expected_mime(&canonical) != Some(asset.mime_type.as_str())
        {
            anyhow::bail!("Demo asset hash, size or MIME does not match its manifest")
        }
    }
    let assets: BTreeMap<_, _> = manifest
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect();
    if manifest.task.source_image_asset_ids.is_empty()
        || manifest.task.source_image_asset_ids.iter().any(|id| {
            assets
                .get(id.as_str())
                .is_none_or(|asset| asset.kind != "image")
        })
        || assets
            .get(manifest.modes.preset_candidates.asset_id.as_str())
            .is_none_or(|asset| asset.kind != "preset_candidates")
        || assets
            .get(manifest.license.attribution_asset_id.as_str())
            .is_none_or(|asset| asset.kind != "attribution")
        || assets
            .get("thumbnail")
            .is_none_or(|asset| asset.kind != "thumbnail")
        || manifest.task.labels.is_empty()
        || manifest
            .task
            .labels
            .iter()
            .any(|label| !valid_id(&label.id) || label.annotation_kind != "bounding_box")
        || manifest.modes.live_model.required_capabilities != ["vision_language"]
    {
        anyhow::bail!("Demo manifest references an invalid task asset or capability")
    }
    for id in &manifest.task.source_image_asset_ids {
        let asset = assets[id.as_str()];
        if asset.width.is_none_or(|value| value == 0) || asset.height.is_none_or(|value| value == 0)
        {
            anyhow::bail!("Demo images require positive declared dimensions")
        }
    }
    validate_preset_candidates(&pack, &manifest, &assets)?;
    Ok(LoadedManifest {
        manifest,
        root: pack,
        manifest_sha256,
    })
}

fn validate_preset_candidates(
    pack: &Path,
    manifest: &RepositoryManifest,
    assets: &BTreeMap<&str, &ManifestAsset>,
) -> anyhow::Result<()> {
    let source = assets[manifest.modes.preset_candidates.asset_id.as_str()];
    let payload: PresetCandidatesFile =
        serde_json::from_slice(&std::fs::read(pack.join(&source.path))?)?;
    if payload.format != "annotagent.demo-candidates"
        || payload.version != 1
        || payload.demo_id != manifest.demo_id
        || payload.demo_version != manifest.version
        || payload.images.len() != manifest.task.source_image_asset_ids.len()
    {
        anyhow::bail!("Preset candidate identity does not match its Demo manifest")
    }
    let labels: BTreeSet<_> = manifest
        .task
        .labels
        .iter()
        .map(|label| label.id.as_str())
        .collect();
    let mut image_ids = BTreeSet::new();
    let mut candidate_ids = BTreeSet::new();
    let mut artifact_ids = BTreeSet::new();
    for image in &payload.images {
        let Some(asset) = assets.get(image.image_asset_id.as_str()) else {
            anyhow::bail!("Preset candidates reference an unknown image")
        };
        if asset.kind != "image"
            || image.image_sha256 != asset.sha256
            || !image_ids.insert(&image.image_asset_id)
        {
            anyhow::bail!("Preset candidate image identity is duplicate or stale")
        }
        for candidate in &image.candidates {
            let rect = &candidate.value;
            if !valid_id(&candidate.id)
                || !candidate_ids.insert(&candidate.id)
                || !valid_id(&candidate.source_artifact_id)
                || !artifact_ids.insert(&candidate.source_artifact_id)
                || !labels.contains(candidate.label_id.as_str())
                || candidate.annotation_kind != "bounding_box"
                || candidate.score.is_some()
                || ![rect.x, rect.y, rect.width, rect.height]
                    .into_iter()
                    .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
                || rect.width <= 0.0
                || rect.height <= 0.0
                || rect.x + rect.width > 1.0
                || rect.y + rect.height > 1.0
            {
                anyhow::bail!("Preset candidate geometry, label or provenance is invalid")
            }
        }
    }
    if image_ids.len() != manifest.task.source_image_asset_ids.len() {
        anyhow::bail!("Preset candidate images do not cover the manifest scope")
    }
    Ok(())
}

fn load_manifest(demo_id: &str, version: &str) -> anyhow::Result<LoadedManifest> {
    load_manifest_from(&catalog_root(), demo_id, version)
}

fn catalog() -> anyhow::Result<(String, Vec<LoadedManifest>)> {
    let manifests = ALLOWLIST
        .iter()
        .map(|(id, version)| load_manifest(id, version))
        .collect::<Result<Vec<_>, _>>()?;
    let revision = format!(
        "{:x}",
        Sha256::digest(
            manifests
                .iter()
                .flat_map(|item| item.manifest_sha256.bytes())
                .collect::<Vec<_>>()
        )
    );
    Ok((revision, manifests))
}

pub(crate) fn validate_repository_catalog() -> anyhow::Result<()> {
    catalog().map(|_| ())
}

fn entry(item: &LoadedManifest) -> DemoCatalogEntry {
    let thumbnail = item
        .manifest
        .assets
        .iter()
        .find(|asset| asset.id == "thumbnail")
        .expect("validated pack has thumbnail");
    let mut modes = Vec::new();
    if item.manifest.modes.preset_candidates.enabled {
        modes.push(DemoModeAvailability {
            source_mode: DemoSourceMode::PresetCandidates,
            status: "ready",
            reason: None,
            required_capabilities: vec![],
        });
    }
    if item.manifest.modes.live_model.enabled {
        modes.push(DemoModeAvailability {
            source_mode: DemoSourceMode::LiveModel,
            status: "setup_required",
            reason: Some(
                "Select a compatible available vision-language Model Profile when starting",
            ),
            required_capabilities: item.manifest.modes.live_model.required_capabilities.clone(),
        });
    }
    DemoCatalogEntry {
        demo_id: item.manifest.demo_id.clone(),
        version: item.manifest.version.clone(),
        manifest_sha256: item.manifest_sha256.clone(),
        title: item.manifest.display.title.clone(),
        summary: item.manifest.display.summary.clone(),
        learning_objectives: item.manifest.display.learning_objectives.clone(),
        image_count: item.manifest.task.source_image_asset_ids.len(),
        labels: item
            .manifest
            .task
            .labels
            .iter()
            .map(|label| label.name.clone())
            .collect(),
        delivery_format: item.manifest.task.training_target.task.clone(),
        thumbnail_asset_id: thumbnail.id.clone(),
        thumbnail_url: format!(
            "/api/demo-catalog/{}/versions/{}/assets/{}",
            item.manifest.demo_id, item.manifest.version, thumbnail.id
        ),
        license: item.manifest.license.clone(),
        modes,
    }
}

pub(crate) async fn list(Query(query): Query<CatalogQuery>) -> ApiResult<Json<DemoCatalogPage>> {
    let offset = query
        .cursor
        .as_deref()
        .unwrap_or("0")
        .parse::<usize>()
        .map_err(|_| {
            unprocessable(
                "invalid_catalog_cursor",
                "catalog cursor must be a non-negative integer",
            )
        })?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(unprocessable(
            "invalid_catalog_limit",
            "catalog limit must be between 1 and 100",
        ));
    }
    let (revision, manifests) = catalog().map_err(ApiError::internal)?;
    if offset > manifests.len() {
        return Err(unprocessable(
            "invalid_catalog_cursor",
            "catalog cursor is beyond the allowlisted catalog",
        ));
    }
    let items = manifests
        .iter()
        .skip(offset)
        .take(limit)
        .map(entry)
        .collect::<Vec<_>>();
    let next = offset + items.len();
    Ok(Json(DemoCatalogPage {
        contract_version: CONTRACT_VERSION,
        catalog_revision: revision,
        items,
        next_cursor: (next < manifests.len()).then(|| next.to_string()),
    }))
}

pub(crate) async fn get_manifest(
    AxumPath((demo_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<PublicManifest>> {
    let item = load_manifest(&demo_id, &version)
        .map_err(|_| ApiError::not_found("Demo manifest was not found"))?;
    let (revision, _) = catalog().map_err(ApiError::internal)?;
    let manifest = item.manifest;
    Ok(Json(PublicManifest {
        contract_version: CONTRACT_VERSION,
        catalog_revision: revision,
        manifest_sha256: item.manifest_sha256,
        schema_version: manifest.schema_version,
        demo_id: manifest.demo_id.clone(),
        version: manifest.version.clone(),
        display: ManifestDisplayPublic {
            title: manifest.display.title,
            summary: manifest.display.summary,
            learning_objectives: manifest.display.learning_objectives,
        },
        project: ManifestProjectPublic {
            display_name: manifest.project.display_name,
            goal: manifest.project.goal,
        },
        assets: manifest
            .assets
            .into_iter()
            .map(|asset| PublicAsset {
                download_url: format!(
                    "/api/demo-catalog/{demo_id}/versions/{version}/assets/{}",
                    asset.id
                ),
                id: asset.id,
                kind: asset.kind,
                sha256: asset.sha256,
                mime_type: asset.mime_type,
                bytes: asset.bytes,
                width: asset.width,
                height: asset.height,
                split: asset.split,
                source_group: asset.source_group,
            })
            .collect(),
        task: PublicTask {
            source_image_asset_ids: manifest.task.source_image_asset_ids,
            labels: manifest.task.labels,
            training_target: manifest.task.training_target,
            split_policy: manifest.task.split_policy,
            review_policy: manifest.task.review_policy,
        },
        modes: PublicModes {
            preset_candidates: PublicPresetMode {
                enabled: manifest.modes.preset_candidates.enabled,
                asset_id: manifest.modes.preset_candidates.asset_id,
            },
            live_model: PublicLiveMode {
                enabled: manifest.modes.live_model.enabled,
                required_capabilities: manifest.modes.live_model.required_capabilities,
            },
        },
        license: manifest.license,
    }))
}

pub(crate) async fn get_asset(
    AxumPath((demo_id, version, asset_id)): AxumPath<(String, String, String)>,
) -> ApiResult<Response<Body>> {
    let item = load_manifest(&demo_id, &version)
        .map_err(|_| ApiError::not_found("Demo asset was not found"))?;
    let asset = item
        .manifest
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| ApiError::not_found("Demo asset was not found"))?;
    let bytes = std::fs::read(item.root.join(&asset.path)).map_err(ApiError::internal)?;
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.mime_type).map_err(ApiError::internal)?,
    );
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{}\"", item.manifest_sha256))
            .map_err(ApiError::internal)?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    Ok(response)
}

fn unprocessable(code: &str, message: &str) -> ApiError {
    ApiError {
        status: StatusCode::UNPROCESSABLE_ENTITY,
        body: serde_json::json!({"code":code,"error":message,"status":422,"suggested_action":"review_request_or_reload_scope"}),
    }
}

fn start_error(error: anyhow::Error) -> ApiError {
    if let Some(StorageError::ConversationContract {
        code: "demo_command_conflict",
        message,
    }) = error.downcast_ref::<StorageError>()
    {
        return ApiError {
            status: StatusCode::CONFLICT,
            body: serde_json::json!({"code":"demo_command_conflict","error":message,"status":409,"suggested_action":"recover_the_original_command_or_choose_a_new_command_id"}),
        };
    }
    ApiError::internal(error)
}

fn project_yaml(manifest: &RepositoryManifest) -> anyhow::Result<String> {
    let value = serde_json::json!({
        "version":1,
        "project":{"name":manifest.project.display_name,"annotation_goal":manifest.project.goal,"language":"zh-CN"},
        "dataset":{"root":"images","include":["**/*.png"],"recursive":true},
        "runtime":{"max_parallel_images":2,"max_model_turns_per_task":4,"max_tool_calls_per_task":6,"max_recovery_turns_per_task":1,"task_timeout_seconds":300,"provider_request_timeout_seconds":120,"max_retries":0,"auto_resume":true},
        "tasks":[{"id":"objects","display_name":"Objects","kind":"bounding_box","labels":manifest.task.labels.iter().map(|label| label.id.as_str()).collect::<Vec<_>>(),"required":true,"multi_label":true}],
        "review":{"auto_accept_confidence":1.0,"force_review_below":1.0,"force_review_on_warning_codes":[]},
        "export":{"formats":["yolo"]}
    });
    let yaml = serde_yaml::to_string(&value)?;
    annotagent_core::ProjectSchema::from_yaml(&yaml).map_err(|error| anyhow::anyhow!(error))?;
    Ok(yaml)
}

fn stable_ids(command: Uuid) -> (Uuid, Uuid, Uuid, Uuid) {
    (
        Uuid::new_v5(&command, b"conversation"),
        Uuid::new_v5(&command, b"task"),
        Uuid::new_v5(&command, b"goal-message"),
        Uuid::new_v5(&command, b"delivery-intent"),
    )
}

fn validate_start_model(state: &ServerState, request: &StartDemoRequest) -> Result<(), ApiError> {
    match (request.source_mode, request.model_profile_id) {
        (DemoSourceMode::PresetCandidates, None) => Ok(()),
        (DemoSourceMode::PresetCandidates, Some(_)) => Err(unprocessable(
            "model_forbidden_for_preset",
            "preset_candidates forbids model_profile_id",
        )),
        (DemoSourceMode::LiveModel, None) => Err(unprocessable(
            "model_profile_required",
            "live_model requires model_profile_id",
        )),
        (DemoSourceMode::LiveModel, Some(id)) => {
            let model = state
                .application
                .store()
                .get_model_profile(id, None)
                .map_err(|_| {
                    unprocessable(
                        "model_profile_unavailable",
                        "Model Profile was not found or is not available",
                    )
                })?;
            if !model.enabled
                || model.status != ModelProfileStatus::Available
                || !model.input_modalities.contains(&InputModality::Image)
                || !model
                    .task_capabilities
                    .contains(&ModelCapability::VisionLanguage)
            {
                return Err(unprocessable(
                    "model_profile_incompatible",
                    "live_model requires an enabled available vision-language Model Profile with image input",
                ));
            }
            Ok(())
        }
    }
}

fn stored_replay(
    state: &ServerState,
    request: &StartDemoRequest,
) -> Result<Option<DemoStartReceipt>, ApiError> {
    let Some((scope, mut receipt)) = state
        .application
        .store()
        .demo_start_receipt(request.command_id)
        .map_err(ApiError::internal)?
    else {
        return Ok(None);
    };
    let input_scope = DemoStartScope {
        command_id: request.command_id,
        demo_id: request.demo_id.clone(),
        demo_version: request.demo_version.clone(),
        source_mode: request.source_mode,
        model_profile_id: request.model_profile_id,
    };
    if scope != input_scope {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: serde_json::json!({"code":"demo_command_conflict","error":"Demo command ID is already bound to another exact scope","status":409,"suggested_action":"recover_the_original_command_or_choose_a_new_command_id"}),
        });
    }
    receipt.replayed = true;
    Ok(Some(receipt))
}

pub(crate) async fn start(
    State(state): State<ServerState>,
    Json(request): Json<StartDemoRequest>,
) -> ApiResult<(StatusCode, Json<DemoStartReceipt>)> {
    if request.command_id.is_nil() {
        return Err(unprocessable(
            "command_id_required",
            "command_id must be a non-nil UUID",
        ));
    }
    if let Some(receipt) = stored_replay(&state, &request)? {
        return Ok((StatusCode::OK, Json(receipt)));
    }
    let item = load_manifest(&request.demo_id, &request.demo_version)
        .map_err(|_| ApiError::not_found("Demo manifest was not found"))?;
    validate_start_model(&state, &request)?;
    let _gate = state.demo_start_gate.lock().await;
    if let Some(receipt) = stored_replay(&state, &request)? {
        return Ok((StatusCode::OK, Json(receipt)));
    }

    let (catalog_revision, _) = catalog().map_err(ApiError::internal)?;
    let route_id = format!("demo-{}-{}", request.demo_id, request.command_id.simple());
    let directory = state.application.workspace().join(&route_id);
    let marker = directory.join(".annotagent-demo-start.json");
    let scope = DemoStartScope {
        command_id: request.command_id,
        demo_id: request.demo_id.clone(),
        demo_version: request.demo_version.clone(),
        source_mode: request.source_mode,
        model_profile_id: request.model_profile_id,
    };
    let scope_json = serde_json::to_string(&scope).map_err(ApiError::internal)?;
    let created = if directory.exists() {
        let saved = std::fs::read_to_string(&marker).map_err(|_| {
            ApiError::internal(
                "incomplete Demo directory does not have a recoverable command marker",
            )
        })?;
        if saved != scope_json {
            return Err(ApiError::internal(
                "Demo directory identity conflicts with its command",
            ));
        }
        false
    } else {
        let yaml = project_yaml(&item.manifest).map_err(ApiError::internal)?;
        state
            .application
            .create_project(&route_id, &yaml)
            .map_err(ApiError::internal)?;
        std::fs::write(&marker, &scope_json).map_err(ApiError::internal)?;
        true
    };
    let result = (|| -> anyhow::Result<(DemoStartReceipt, bool)> {
        let image_assets = item
            .manifest
            .task
            .source_image_asset_ids
            .iter()
            .map(|id| {
                item.manifest
                    .assets
                    .iter()
                    .find(|asset| &asset.id == id)
                    .expect("validated image reference")
            })
            .collect::<Vec<_>>();
        let mut image_seeds = Vec::new();
        let mut delivery_images = Vec::new();
        let project_path = directory.canonicalize()?;
        let owner = stable_project_id(&project_path);
        for asset in image_assets {
            let extension = Path::new(&asset.path)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("png");
            let relative_path = format!("images/{}.{}", asset.id, extension);
            let destination = directory.join(&relative_path);
            if !destination.exists() {
                std::fs::copy(item.root.join(&asset.path), &destination)?;
            }
            let metadata_json = serde_json::json!({"demo_id":request.demo_id,"demo_version":request.demo_version,"source_asset_id":asset.id,"width":asset.width,"height":asset.height}).to_string();
            let image_id = ImageId(Uuid::new_v5(&owner.0, relative_path.as_bytes()));
            let split = match asset.split.as_deref() {
                Some("train") => Some(DatasetSplit::Train),
                Some("validation") => Some(DatasetSplit::Val),
                Some("test") => Some(DatasetSplit::Test),
                _ => None,
            };
            delivery_images.push(DeliveryImage {
                image_id,
                content_sha256: asset.sha256.clone(),
                content_revision: asset.sha256.clone(),
                existing_split: split,
                group_ids: asset.source_group.clone().into_iter().collect(),
            });
            image_seeds.push(DemoImageSeed {
                relative_path,
                sha256: asset.sha256.clone(),
                metadata_json,
            });
        }
        let yaml = std::fs::read(directory.join("project.yaml"))?;
        let schema_revision = format!("{:x}", Sha256::digest(&yaml));
        let (conversation_id, task_id, source_message_id, delivery_command_id) =
            stable_ids(request.command_id);
        let preset_asset = item
            .manifest
            .assets
            .iter()
            .find(|asset| asset.id == item.manifest.modes.preset_candidates.asset_id)
            .expect("validated preset asset");
        let preset_payload = (request.source_mode == DemoSourceMode::PresetCandidates)
            .then(
                || -> anyhow::Result<(serde_json::Value, PresetCandidatesFile)> {
                    let bytes = std::fs::read(item.root.join(&preset_asset.path))?;
                    Ok((
                        serde_json::from_slice(&bytes)?,
                        serde_json::from_slice(&bytes)?,
                    ))
                },
            )
            .transpose()?;
        let preset_candidates = preset_payload.as_ref().map(|value| value.0.clone());
        let preset_annotations = preset_payload
            .as_ref()
            .into_iter()
            .flat_map(|value| &value.1.images)
            .flat_map(|image| {
                let relative_path = format!("images/{}.png", image.image_asset_id);
                let image_id = ImageId(Uuid::new_v5(&owner.0, relative_path.as_bytes()));
                image.candidates.iter().map(move |candidate| {
                    let annotation = Annotation {
                        id: AnnotationId(Uuid::new_v5(
                            &owner.0,
                            format!("preset-candidate:{}", candidate.id).as_bytes(),
                        )),
                        image_id,
                        task_id: TaskId::from("objects"),
                        label: Some(candidate.label_id.as_str().into()),
                        value: AnnotationValue::BoundingBox {
                            rect: NormalizedRect::new(
                                candidate.value.x as f32,
                                candidate.value.y as f32,
                                candidate.value.width as f32,
                                candidate.value.height as f32,
                            )
                            .expect("validated preset candidate geometry"),
                        },
                        attributes: BTreeMap::new(),
                        confidence: None,
                        source: AnnotationSource::Imported,
                        review_status: ReviewStatus::NeedsReview,
                        provenance: AnnotationProvenance {
                            artifact_ids: vec![ArtifactId(Uuid::new_v5(
                                &owner.0,
                                candidate.source_artifact_id.as_bytes(),
                            ))],
                            ..AnnotationProvenance::default()
                        },
                        created_at: chrono::Utc::now(),
                    };
                    DemoPresetAnnotationSeed {
                        annotation,
                        source_artifact_id: candidate.source_artifact_id.clone(),
                    }
                })
            })
            .collect();
        let provenance = if request.source_mode == DemoSourceMode::PresetCandidates {
            DemoSourceProvenance {
                kind: request.source_mode,
                live_inference_occurred: false,
                review_status: Some("needs_review".into()),
                source_asset_id: Some(preset_asset.id.clone()),
                source_asset_sha256: Some(preset_asset.sha256.clone()),
            }
        } else {
            DemoSourceProvenance {
                kind: request.source_mode,
                live_inference_occurred: false,
                review_status: None,
                source_asset_id: None,
                source_asset_sha256: None,
            }
        };
        let receipt = DemoStartReceipt {
            contract_version: START_CONTRACT_VERSION.into(),
            command_id: request.command_id,
            demo_id: request.demo_id.clone(),
            demo_version: request.demo_version.clone(),
            source_mode: request.source_mode,
            catalog_revision,
            manifest_sha256: item.manifest_sha256.clone(),
            status: DemoStartStatus::Ready,
            project_id: route_id.clone(),
            project_owner_id: owner.to_string(),
            conversation_id,
            task_id,
            work_route: format!(
                "/projects/{route_id}/work?conversation={conversation_id}&task={task_id}"
            ),
            source_provenance: provenance,
            replayed: false,
            retry_safe: true,
            detail: None,
        };
        let intent = TaskDeliveryIntent {
            version: 1,
            project_id: owner.to_string(),
            conversation_id,
            task_id,
            dataset_scope: Some(delivery_images),
            label_spec: Some(
                item.manifest
                    .task
                    .labels
                    .iter()
                    .map(|label| DeliveryLabel {
                        stable_id: label.id.clone(),
                        display_name: label.name.clone(),
                        aliases: vec![],
                        include: format!("Visible instances of {}", label.name),
                        exclude: format!("Do not label non-{} objects", label.name),
                    })
                    .collect(),
            ),
            training_target: Some(TrainingTarget {
                annotation_kind: TaskKind::BoundingBox,
                framework: item.manifest.task.training_target.framework.clone(),
                export_profile: item.manifest.task.training_target.task.clone(),
                profile_revision: item.manifest.task.training_target.format_revision,
            }),
            split_policy: DeliverySplitPolicy {
                train_percent: (item.manifest.task.split_policy.train * 100.0).round() as u8,
                seed: item.manifest.task.split_policy.seed,
                preserve_existing: true,
                keep_known_groups_together: true,
            },
            review_policy: DeliveryReviewPolicy::HumanWholeImage,
        };
        Ok(state
            .application
            .store()
            .start_demo_context(&DemoStartSeed {
                scope,
                receipt,
                source_message_id,
                goal: item.manifest.project.goal.clone(),
                schema_revision,
                images: image_seeds,
                delivery_intent: intent,
                delivery_command_id,
                model_profile_id: request.model_profile_id,
                preset_candidates,
                preset_annotations,
            })?)
    })();
    match result {
        Ok((receipt, replayed)) => Ok((
            if replayed {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            },
            Json(receipt),
        )),
        Err(error) => {
            if created {
                let _ = std::fs::remove_dir_all(&directory);
            }
            Err(start_error(error))
        }
    }
}

pub(crate) async fn receipt(
    State(state): State<ServerState>,
    AxumPath(command_id): AxumPath<Uuid>,
) -> ApiResult<Json<DemoStartReceipt>> {
    let Some((_scope, mut receipt)) = state
        .application
        .store()
        .demo_start_receipt(command_id)
        .map_err(ApiError::internal)?
    else {
        return Err(ApiError::not_found("Demo command was not found"));
    };
    receipt.replayed = true;
    Ok(Json(receipt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::{
        CapabilityDeclarationSource, GenerationDefaults, ModelLimits, ModelPricing, ModelProfile,
        ProtocolFeatures,
    };
    use annotagent_provider::InMemorySecretStore;
    use axum::http::Method;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn repository_pack_is_exact_and_public_manifest_has_no_paths() {
        let (revision, packs) = catalog().expect("allowlisted catalog");
        assert_eq!(revision.len(), 64);
        assert_eq!(packs.len(), 1);
        let public = entry(&packs[0]);
        assert_eq!(public.image_count, 6);
        assert_eq!(public.labels, ["cup", "bottle"]);
        assert!(public.thumbnail_url.starts_with("/api/demo-catalog/"));
    }

    #[cfg(unix)]
    #[test]
    fn loader_rejects_symlinked_and_changed_assets() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("catalog");
        let pack = root.join("object-detection-review/1.0.0");
        std::fs::create_dir_all(pack.parent().unwrap()).unwrap();
        symlink(catalog_root().join("object-detection-review/1.0.0"), &pack).unwrap();
        assert!(load_manifest_from(&root, "object-detection-review", "1.0.0").is_err());
    }

    fn copy_tree(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    #[test]
    fn loader_rejects_hash_traversal_and_unknown_manifest_fields() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("catalog");
        let pack = root.join("object-detection-review/1.0.0");
        copy_tree(&catalog_root().join("object-detection-review/1.0.0"), &pack);
        let manifest_path = pack.join("manifest.json");
        let original = std::fs::read(&manifest_path).unwrap();

        let mut changed: serde_json::Value = serde_json::from_slice(&original).unwrap();
        changed["assets"][0]["sha256"] = json!("0".repeat(64));
        std::fs::write(&manifest_path, serde_json::to_vec_pretty(&changed).unwrap()).unwrap();
        assert!(load_manifest_from(&root, "object-detection-review", "1.0.0").is_err());

        let mut traversal: serde_json::Value = serde_json::from_slice(&original).unwrap();
        traversal["assets"][0]["path"] = json!("../thumbnail.png");
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&traversal).unwrap(),
        )
        .unwrap();
        assert!(load_manifest_from(&root, "object-detection-review", "1.0.0").is_err());

        let mut unknown: serde_json::Value = serde_json::from_slice(&original).unwrap();
        unknown["unexpected"] = json!(true);
        std::fs::write(&manifest_path, serde_json::to_vec_pretty(&unknown).unwrap()).unwrap();
        assert!(load_manifest_from(&root, "object-detection-review", "1.0.0").is_err());
    }

    async fn test_service(
        workspace: &Path,
    ) -> (Arc<annotagent_application::LocalApplication>, axum::Router) {
        let application = Arc::new(
            annotagent_application::LocalApplication::new(workspace).expect("TEST application"),
        );
        let state = crate::tests::test_state(
            application.clone(),
            Arc::new(InMemorySecretStore::default()),
        )
        .await;
        let service = crate::router(state, None);
        (application, service)
    }

    #[tokio::test]
    async fn catalog_get_is_passive_and_preset_start_is_concurrent_idempotent_and_restart_safe() {
        let temp = tempfile::tempdir().unwrap();
        let (application, service) = test_service(temp.path()).await;
        assert!(application.store().list_runs().unwrap().is_empty());

        let page =
            crate::tests::request(&service, Method::GET, "/api/demo-catalog?limit=1", None).await;
        assert_eq!(page.status(), StatusCode::OK);
        let page = crate::tests::response_json(page).await;
        assert_eq!(page["contract_version"], CONTRACT_VERSION);
        assert_eq!(page["items"][0]["demo_id"], "object-detection-review");
        assert_eq!(page["items"][0]["image_count"], 6);
        assert!(application.store().list_runs().unwrap().is_empty());
        assert!(application.list_workflow_drafts(None).unwrap().is_empty());

        let manifest = crate::tests::response_json(
            crate::tests::request(
                &service,
                Method::GET,
                "/api/demo-catalog/object-detection-review/versions/1.0.0",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(manifest["assets"][0].get("path"), None);
        assert!(manifest["assets"][0]["download_url"].is_string());
        let asset = crate::tests::request(
            &service,
            Method::GET,
            "/api/demo-catalog/object-detection-review/versions/1.0.0/assets/thumbnail",
            None,
        )
        .await;
        assert_eq!(asset.status(), StatusCode::OK);
        assert!(asset.headers().get(header::ETAG).is_some());

        let command = Uuid::new_v4();
        let request = json!({
            "command_id": command,
            "demo_id":"object-detection-review",
            "demo_version":"1.0.0",
            "source_mode":"preset_candidates",
            "model_profile_id":null
        });
        let (first, second) = tokio::join!(
            crate::tests::request(
                &service,
                Method::POST,
                "/api/demos/start",
                Some(request.clone())
            ),
            crate::tests::request(
                &service,
                Method::POST,
                "/api/demos/start",
                Some(request.clone())
            )
        );
        assert_eq!(
            BTreeSet::from([first.status(), second.status()]),
            BTreeSet::from([StatusCode::OK, StatusCode::CREATED])
        );
        let first = crate::tests::response_json(first).await;
        let second = crate::tests::response_json(second).await;
        assert_eq!(first["project_id"], second["project_id"]);
        assert_eq!(first["conversation_id"], second["conversation_id"]);
        assert_eq!(first["task_id"], second["task_id"]);
        assert_eq!(first["source_provenance"]["review_status"], "needs_review");
        assert_eq!(first["source_provenance"]["live_inference_occurred"], false);
        let route = first["project_id"].as_str().unwrap();
        let owner = first["project_owner_id"].as_str().unwrap();
        let conversation = Uuid::parse_str(first["conversation_id"].as_str().unwrap()).unwrap();
        let task = Uuid::parse_str(first["task_id"].as_str().unwrap()).unwrap();
        assert_eq!(
            application
                .conversation_tasks(route, conversation)
                .unwrap()
                .len(),
            1
        );
        let delivery = application
            .store()
            .task_delivery_intent(owner, conversation, task)
            .unwrap()
            .unwrap();
        assert_eq!(delivery.intent.dataset_scope.unwrap().len(), 6);
        assert!(
            application
                .store()
                .demo_preset_candidate_import(owner, task)
                .unwrap()
                .is_some()
        );
        let usage = application
            .task_model_usage(route, conversation, task, 0, 50)
            .unwrap();
        assert_eq!(usage["attempts"]["items"], json!([]));
        assert!(
            application
                .list_workflow_drafts(Some(route))
                .unwrap()
                .is_empty()
        );
        assert!(application.store().list_runs().unwrap().is_empty());

        let task_root = format!("/api/projects/{route}/conversations/{conversation}/tasks/{task}");
        let workspace = crate::tests::response_json(
            crate::tests::request(
                &service,
                Method::GET,
                &format!("{task_root}/workspace"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            workspace["mainline"]["available_actions"][0]["id"],
            "review_delivery_images"
        );
        assert_eq!(
            workspace["mainline"]["formal_source"]["kind"],
            "preset_candidate_import"
        );
        assert_eq!(
            workspace["mainline"]["formal_source"]["live_inference_occurred"],
            false
        );
        let review_page = crate::tests::request(
            &service,
            Method::GET,
            &format!("{task_root}/delivery-review-items?limit=6"),
            None,
        )
        .await;
        assert_eq!(review_page.status(), StatusCode::OK);
        let review_page = crate::tests::response_json(review_page).await;
        assert_eq!(review_page["summary"]["selected"], 6);
        assert_eq!(review_page["summary"]["unreviewed"], 6);
        let first_item = &review_page["items"][0];
        assert!(first_item["url"].as_str().unwrap().ends_with("/content"));
        assert_eq!(first_item["annotations"][0]["origin"], "preset_candidate");
        assert_eq!(
            first_item["annotations"][0]["source_artifact_id"],
            "preset-artifact-image-01-cup-01"
        );
        assert_eq!(
            first_item["annotations"][0]["review_status"],
            "needs_review"
        );
        assert_eq!(first_item["child_run_id"], serde_json::Value::Null);
        let image_id = first_item["image_id"].as_str().unwrap();
        let annotation = &first_item["annotations"][0];
        let review_command = Uuid::new_v4();
        let preset_review_input = json!({
            "command_id":review_command,
            "intent_revision":review_page["intent_revision"],
            "intent_sha256":review_page["intent_sha256"],
            "annotation_id":annotation["annotation_id"],
            "expected_snapshot_sha256":first_item["snapshot_sha256"],
            "label":annotation["label"],
            "value":annotation["value"],
            "review_status":"human_accepted",
            "reason":"TEST human inspected imported candidate"
        });
        let reviewed = crate::tests::request(
            &service,
            Method::POST,
            &format!("{task_root}/delivery-images/{image_id}/preset-objects"),
            Some(preset_review_input.clone()),
        )
        .await;
        assert_eq!(reviewed.status(), StatusCode::OK);
        let reviewed = crate::tests::response_json(reviewed).await;
        let replayed_review = crate::tests::response_json(
            crate::tests::request(
                &service,
                Method::POST,
                &format!("{task_root}/delivery-images/{image_id}/preset-objects"),
                Some(preset_review_input.clone()),
            )
            .await,
        )
        .await;
        assert_eq!(replayed_review, reviewed);
        let mut changed_review = preset_review_input;
        changed_review["reason"] = json!("TEST changed reuse");
        let changed_review = crate::tests::request(
            &service,
            Method::POST,
            &format!("{task_root}/delivery-images/{image_id}/preset-objects"),
            Some(changed_review),
        )
        .await;
        assert_eq!(changed_review.status(), StatusCode::BAD_REQUEST);
        let image = crate::tests::response_json(
            crate::tests::request(
                &service,
                Method::GET,
                &format!("{task_root}/delivery-images/{image_id}"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(image["snapshot"]["annotations"][0]["source"], "imported");
        assert_eq!(
            image["snapshot"]["annotations"][0]["review_status"],
            "human_accepted"
        );
        let confirmed = crate::tests::request(
            &service,
            Method::POST,
            &format!("{task_root}/delivery-images/{image_id}"),
            Some(json!({
                "command_id":Uuid::new_v4(),
                "intent_revision":review_page["intent_revision"],
                "intent_sha256":review_page["intent_sha256"],
                "image_id":image_id,
                "source_run_id":null,
                "expected_snapshot_sha256":image["snapshot"]["sha256"],
                "expected_review_revision":0,
                "decision":"positive_complete",
                "reason":"TEST whole image inspected",
                "confirmed":true
            })),
        )
        .await;
        assert_eq!(confirmed.status(), StatusCode::OK);
        let confirmed = crate::tests::response_json(confirmed).await;
        let package = annotagent_storage::DeliveryPackageInput {
            command_id: Uuid::new_v4(),
            intent_revision: review_page["intent_revision"].as_u64().unwrap() as u32,
            intent_sha256: review_page["intent_sha256"].as_str().unwrap().into(),
            image_reviews: BTreeMap::from([(
                image_id.parse().unwrap(),
                confirmed["revision"].as_u64().unwrap() as u32,
            )]),
            confirmed: true,
        };
        assert!(
            application
                .store()
                .begin_delivery_package(owner, conversation, task, &package)
                .unwrap_err()
                .to_string()
                .contains("every scoped image")
        );
        assert!(application.store().list_runs().unwrap().is_empty());

        let conflict = crate::tests::request(
            &service,
            Method::POST,
            "/api/demos/start",
            Some(json!({"command_id":command,"demo_id":"object-detection-review","demo_version":"1.0.0","source_mode":"live_model","model_profile_id":Uuid::new_v4()})),
        )
        .await;
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert_eq!(
            crate::tests::response_json(conflict).await["code"],
            "demo_command_conflict"
        );

        drop(service);
        drop(application);
        let (_restarted, service) = test_service(temp.path()).await;
        let recovered = crate::tests::request(
            &service,
            Method::GET,
            &format!("/api/demos/start/{command}"),
            None,
        )
        .await;
        assert_eq!(recovered.status(), StatusCode::OK);
        let recovered = crate::tests::response_json(recovered).await;
        assert_eq!(recovered["task_id"], first["task_id"]);
        assert_eq!(recovered["replayed"], true);
        let recovered_review = crate::tests::response_json(
            crate::tests::request(
                &service,
                Method::GET,
                &format!("{task_root}/delivery-review-items?limit=6"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(recovered_review["summary"]["positive"], 1);
        assert_eq!(recovered_review["summary"]["unreviewed"], 5);
        assert_eq!(
            recovered_review["items"][0]["annotations"][0]["review_status"],
            "human_accepted"
        );
        assert_eq!(recovered_review["items"][0]["confirmation_current"], true);
    }

    #[tokio::test]
    async fn live_start_freezes_compatible_profile_without_candidates_or_execution() {
        let temp = tempfile::tempdir().unwrap();
        let (application, service) = test_service(temp.path()).await;
        let provider = application
            .store()
            .list_provider_profiles()
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let now = chrono::Utc::now();
        let model = ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id: provider.id,
            display_name: "TEST Demo VLM".into(),
            remote_model_id: "TEST-demo-vlm".into(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures {
                tool_calls: true,
                structured_output: true,
                ..ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([
                ModelCapability::TextGeneration,
                ModelCapability::VisionLanguage,
            ]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing::default(),
            quality_contracts: vec![],
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        application.store().save_model_profile(&model).unwrap();
        let command = Uuid::new_v4();
        let response = crate::tests::request(
            &service,
            Method::POST,
            "/api/demos/start",
            Some(json!({"command_id":command,"demo_id":"object-detection-review","demo_version":"1.0.0","source_mode":"live_model","model_profile_id":model.id})),
        ).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let receipt = crate::tests::response_json(response).await;
        let route = receipt["project_id"].as_str().unwrap();
        let owner = receipt["project_owner_id"].as_str().unwrap();
        let conversation = Uuid::parse_str(receipt["conversation_id"].as_str().unwrap()).unwrap();
        let task = Uuid::parse_str(receipt["task_id"].as_str().unwrap()).unwrap();
        assert_eq!(receipt["source_provenance"]["kind"], "live_model");
        assert_eq!(
            receipt["source_provenance"]["live_inference_occurred"],
            false
        );
        assert!(
            application
                .store()
                .demo_preset_candidate_import(owner, task)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            application
                .project_conversation_agent_model(route, conversation)
                .unwrap()
                .model_profile_id,
            Some(model.id)
        );
        let usage = application
            .task_model_usage(route, conversation, task, 0, 50)
            .unwrap();
        assert_eq!(usage["attempts"]["items"], json!([]));
        assert!(
            application
                .list_workflow_drafts(Some(route))
                .unwrap()
                .is_empty()
        );
        assert!(application.store().list_runs().unwrap().is_empty());
    }
}
