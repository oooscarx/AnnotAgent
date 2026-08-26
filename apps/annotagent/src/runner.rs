use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use annotagent_core::{
    Budget, DomainSkill, ImageId, PricingConfig, ProjectId, ProjectSchema, RunId,
    ValidationCatalog, VisionModelProvider,
};
use annotagent_image_tools::{generate_synthetic_robocup, load_image, to_model_image};
use annotagent_provider::{
    MockResponseSpec, MockScript, MockStep, MockUsage, MockVisionProvider, OpenAiCompatibleConfig,
    OpenAiCompatibleProvider,
};
use annotagent_runtime::{AgentLoopConfig, AgentRuntime, ImageRunRequest};
use annotagent_skill_robocup::RoboCupSkill;
use annotagent_storage::SqliteStore;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::json;
use walkdir::WalkDir;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub provider: OpenAiCompatibleConfig,
    pub pricing: PricingConfig,
    pub budget: Budget,
}

pub struct PreparedRun {
    pub runtime: Arc<AgentRuntime>,
    pub request: ImageRunRequest,
    pub image_path: PathBuf,
}

pub fn load_project(path: &Path) -> Result<(ProjectSchema, Arc<dyn DomainSkill>)> {
    let yaml = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read project {}", path.display()))?;
    let project = ProjectSchema::from_yaml(&yaml).map_err(|issue| anyhow::anyhow!(issue))?;
    let skill: Arc<dyn DomainSkill> = match project.project.skill.as_str() {
        "robocup" => Arc::new(RoboCupSkill::new().map_err(|error| anyhow::anyhow!(error))?),
        other => bail!(
            "skill {other:?} is not registered; run `annotagent skills list` to inspect installed skills"
        ),
    };
    let catalog = ValidationCatalog {
        validators: skill
            .validators()
            .into_iter()
            .map(|validator| validator.id().to_owned())
            .collect(),
        refiners: skill
            .refiners()
            .into_iter()
            .map(|refiner| refiner.id().to_owned())
            .collect(),
    };
    let issues = project.validate(&catalog);
    if !issues.is_empty() {
        let details = issues
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        bail!("project validation failed:\n{details}");
    }
    Ok((project, skill))
}

pub fn load_settings(path: Option<&Path>) -> Result<Settings> {
    let contents = if let Some(path) = path {
        std::fs::read_to_string(path)
            .with_context(|| format!("cannot read config {}", path.display()))?
    } else {
        include_str!("../../../config/default.toml").to_owned()
    };
    toml::from_str(&contents).context("invalid provider/pricing/budget config")
}

pub fn prepare_run(
    project_path: &Path,
    provider_kind: &str,
    config_path: Option<&Path>,
) -> Result<PreparedRun> {
    let (project, skill) = load_project(project_path)?;
    let settings = load_settings(config_path)?;
    let image_path = find_or_generate_image(project_path, &project)?;
    let image =
        Arc::new(load_image(&image_path, 40_000_000).map_err(|error| anyhow::anyhow!(error))?);
    let model_image =
        to_model_image("full-image", &image, 1280).map_err(|error| anyhow::anyhow!(error))?;
    let provider: Arc<dyn VisionModelProvider> = match provider_kind {
        "mock" => Arc::new(MockVisionProvider::new(mock_script(
            &project,
            skill.as_ref(),
        )?)),
        "openai_compatible" => Arc::new(
            OpenAiCompatibleProvider::new(settings.provider.clone())
                .map_err(|error| anyhow::anyhow!(error))?,
        ),
        other => bail!("unknown provider {other:?}; choose mock or openai_compatible"),
    };
    let database = database_path()?;
    if let Some(parent) = database.parent() {
        std::fs::create_dir_all(parent).context("cannot create .annotagent directory")?;
    }
    let store = Arc::new(SqliteStore::open(&database).context("cannot open run history")?);
    let runtime = Arc::new(AgentRuntime::new(
        skill,
        provider,
        store.clone(),
        settings.pricing,
        settings.budget,
        AgentLoopConfig {
            model: settings.provider.model,
            max_steps_per_image: project.runtime.max_agent_steps_per_image,
            max_retries_per_task: project.runtime.max_retries_per_task,
            max_output_tokens: settings.provider.max_output_tokens,
            temperature: settings.provider.temperature,
            ..AgentLoopConfig::default()
        },
    ));
    let project_root = project_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .context("cannot canonicalize project root")?;
    Ok(PreparedRun {
        runtime,
        request: ImageRunRequest {
            run_id: RunId::new(),
            project_id: ProjectId::new(),
            project_root,
            project: Arc::new(project),
            image_id: ImageId::new(),
            image,
            model_image: Some(model_image),
        },
        image_path,
    })
}

pub fn database_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()
        .context("cannot determine current directory")?
        .join(".annotagent/history.db"))
}

fn find_or_generate_image(project_path: &Path, project: &ProjectSchema) -> Result<PathBuf> {
    let root = project_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&project.dataset.root);
    std::fs::create_dir_all(&root)
        .with_context(|| format!("cannot create dataset root {}", root.display()))?;
    if let Some(path) = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .find(|path| is_supported_image(path))
    {
        return Ok(path);
    }
    if project.project.skill != "robocup" {
        bail!("dataset has no supported image; import an image before running");
    }
    let path = root.join("synthetic-robocup.png");
    generate_synthetic_robocup(&path).map_err(|error| anyhow::anyhow!(error))?;
    Ok(path)
}

pub fn is_supported_image(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png"
                )
            })
}

fn mock_script(project: &ProjectSchema, skill: &dyn DomainSkill) -> Result<MockScript> {
    let known: std::collections::BTreeSet<_> =
        project.tasks.iter().map(|task| task.id.as_str()).collect();
    let ordered = skill
        .workflow()
        .topological_order()
        .map_err(|error| anyhow::anyhow!("invalid skill workflow: {error}"))?;
    Ok(MockScript {
        steps: ordered
            .into_iter()
            .filter(|task| known.contains(task.as_str()))
            .filter_map(|task| mock_step(task.as_str()))
            .collect(),
    })
}

fn mock_step(task: &str) -> Option<MockStep> {
    let annotations = match task {
        "scene_type" => json!([{
            "label": "normal_field",
            "value": {"kind": "classification", "labels": ["normal_field"]},
            "attributes": {}, "confidence": 0.99
        }]),
        "field_region" => json!([{
            "label": "field",
            "value": {"kind": "polygon", "rings": [[[0.02, 0.02], [0.98, 0.02], [0.98, 0.98], [0.02, 0.98]]]},
            "attributes": {}, "confidence": 0.98
        }]),
        "field_line" => json!([{
            "label": "white_field_line",
            "value": {"kind": "polyline", "points": [[0.08, 0.47], [0.92, 0.47]]},
            "attributes": {}, "confidence": 0.96
        }]),
        "penalty_mark" => json!([{
            "label": "penalty_mark",
            "value": {"kind": "keypoints", "points": [{"name": "center", "point": [0.775, 0.695], "visible": true}]},
            "attributes": {}, "confidence": 0.97
        }]),
        "objects" => json!([
            {
                "label": "robot",
                "value": {"kind": "bounding_box", "rect": [0.225, 0.445, 0.07, 0.2]},
                "attributes": {}, "confidence": 0.98
            },
            {
                "label": "ball",
                "value": {"kind": "bounding_box", "rect": [0.547, 0.75, 0.038, 0.06]},
                "attributes": {}, "confidence": 0.98
            }
        ]),
        "robot_attributes" => json!([{
            "label": "robot",
            "value": {"kind": "bounding_box", "rect": [0.225, 0.445, 0.07, 0.2]},
            "attributes": {"team_color": "red", "state": "standing"},
            "confidence": 0.98
        }]),
        _ => return None,
    };
    Some(MockStep {
        expect_task: Some(task.to_owned()),
        expect_message_contains: None,
        response: MockResponseSpec::ToolCall {
            name: "submit_annotation_candidates".to_owned(),
            arguments: json!({"annotations": annotations}),
        },
        usage: MockUsage {
            input_tokens: 180,
            output_tokens: 45,
        },
    })
}
