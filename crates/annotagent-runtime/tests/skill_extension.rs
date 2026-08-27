use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    AgentTool, AnnotationRefiner, AnnotationValidator, CoreResult, CorrectionKind, DomainSkill,
    ReviewContext, ReviewDecision, ReviewPolicy, Skill, SkillDependency, SkillKind, SkillManifest,
    SkillResource, SkillResourceRequest, TaskGraph, TaskId, TaskNode, TaskTemplate, ToolContext,
    ToolDefinition, ToolResult, ValidationContext, ValidationIssue,
};
use annotagent_runtime::{LayeredSkillRegistry, RegistryError, SkillRegistry, ToolRegistry};
use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

struct DummyTool;

#[async_trait]
impl AgentTool for DummyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "submit_dummy_classification".to_owned(),
            description: "Submit a classification from an external skill".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {"label": {"type": "string"}},
                "required": ["label"],
                "additionalProperties": false
            }),
            read_only: false,
        }
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        arguments: serde_json::Value,
    ) -> CoreResult<ToolResult> {
        Ok(ToolResult::structured(
            "classification submitted",
            arguments,
        ))
    }
}

struct DummyReview;

impl ReviewPolicy for DummyReview {
    fn decide(&self, _context: &ReviewContext<'_>) -> ReviewDecision {
        ReviewDecision::AutoAccept {
            reasons: vec!["dummy policy".to_owned()],
        }
    }
}

struct DummyValidator;

impl AnnotationValidator for DummyValidator {
    fn id(&self) -> &str {
        "check"
    }

    fn validate(&self, _context: &ValidationContext<'_>) -> CoreResult<Vec<ValidationIssue>> {
        Ok(Vec::new())
    }
}

struct DummySkill {
    manifest: SkillManifest,
}

impl DummySkill {
    fn new() -> Self {
        Self::named("dummy", json!({"color": "#3366ff"}))
    }

    fn named(id: &str, visual: serde_json::Value) -> Self {
        Self {
            manifest: SkillManifest {
                version: 1,
                id: id.to_owned(),
                kind: SkillKind::Domain,
                skill_version: "1".to_owned(),
                display_name: "Dummy external skill".to_owned(),
                description: "Proves the runtime has no domain-specific branch".to_owned(),
                rust_implementation: None,
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                capabilities: Vec::new(),
                nodes: Vec::new(),
                tools: Vec::new(),
                validators: Vec::new(),
                policies: Vec::new(),
                templates: Vec::new(),
                summary_resources: vec!["summary".to_owned()],
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
                visual_profile: BTreeMap::from([("target".to_owned(), visual)]),
            },
        }
    }
}

struct LayeredDummy {
    manifest: SkillManifest,
}

impl LayeredDummy {
    fn new(id: &str, kind: SkillKind, dependencies: Vec<SkillDependency>) -> Self {
        let mut manifest = DummySkill::named(id, json!({})).manifest;
        manifest.kind = kind;
        manifest.dependencies = dependencies;
        manifest.capabilities = vec![format!("{id}.ability")];
        Self { manifest }
    }
}

impl Skill for LayeredDummy {
    fn id(&self) -> &str {
        &self.manifest.id
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        Ok(request
            .resource_name
            .iter()
            .map(|name| SkillResource {
                name: name.clone(),
                media_type: "text/markdown".to_owned(),
                content: "bounded context".to_owned(),
            })
            .collect())
    }
}

#[test]
fn layered_registry_resolves_capability_domain_and_pack_dependencies() {
    let mut registry = LayeredSkillRegistry::new();
    registry
        .register(Arc::new(LayeredDummy::new(
            "classification",
            SkillKind::Capability,
            Vec::new(),
        )))
        .expect("Capability Skill");
    registry
        .register(Arc::new(LayeredDummy::new(
            "example.object",
            SkillKind::Domain,
            vec![SkillDependency {
                id: "classification".to_owned(),
                version: "1".to_owned(),
            }],
        )))
        .expect("Domain Skill");
    registry
        .register(Arc::new(LayeredDummy::new(
            "example",
            SkillKind::Pack,
            vec![SkillDependency {
                id: "example.object".to_owned(),
                version: "1".to_owned(),
            }],
        )))
        .expect("Pack Skill");

    let enabled = BTreeMap::from([
        ("classification".to_owned(), "1".to_owned()),
        ("example.object".to_owned(), "1".to_owned()),
        ("example".to_owned(), "1".to_owned()),
    ]);
    assert_eq!(
        registry.resolve_enabled(&enabled).expect("resolved").len(),
        3
    );
    assert_eq!(registry.catalog().len(), 3);

    let missing = BTreeMap::from([("example.object".to_owned(), "1".to_owned())]);
    assert!(matches!(
        registry.resolve_enabled(&missing),
        Err(RegistryError::MissingDependency { .. })
    ));
}

#[test]
fn layered_registry_rejects_conflicts_and_resource_traversal() {
    let mut left = LayeredDummy::new("left", SkillKind::Domain, Vec::new());
    left.manifest.conflicts.push("right".to_owned());
    left.manifest.summary_resources.push("guide.md".to_owned());
    let mut registry = LayeredSkillRegistry::new();
    registry.register(Arc::new(left)).expect("left");
    registry
        .register(Arc::new(LayeredDummy::new(
            "right",
            SkillKind::Domain,
            Vec::new(),
        )))
        .expect("right");
    let enabled = BTreeMap::from([
        ("left".to_owned(), "1".to_owned()),
        ("right".to_owned(), "1".to_owned()),
    ]);
    assert!(matches!(
        registry.resolve_enabled(&enabled),
        Err(RegistryError::Conflict { .. })
    ));
    assert!(matches!(
        registry.load_resource(
            "left",
            &SkillResourceRequest {
                task_id: None,
                resource_name: Some("../secret".to_owned()),
            }
        ),
        Err(RegistryError::UnsafeResource(_))
    ));
    assert_eq!(
        registry
            .load_resource(
                "left",
                &SkillResourceRequest {
                    task_id: None,
                    resource_name: Some("guide.md".to_owned()),
                }
            )
            .expect("declared resource")[0]
            .content,
        "bounded context"
    );
}

impl DomainSkill for DummySkill {
    fn id(&self) -> &str {
        &self.manifest.id
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn task_templates(&self) -> Vec<TaskTemplate> {
        vec![TaskTemplate {
            id: TaskId::from("mood"),
            description: "classify mood".to_owned(),
        }]
    }

    fn workflow(&self) -> TaskGraph {
        TaskGraph {
            nodes: vec![TaskNode {
                id: TaskId::from("mood"),
                depends_on: Vec::new(),
            }],
        }
    }

    fn tool_factories(&self) -> Vec<Arc<dyn AgentTool>> {
        vec![Arc::new(DummyTool)]
    }

    fn validators(&self) -> Vec<Arc<dyn AnnotationValidator>> {
        vec![Arc::new(DummyValidator)]
    }

    fn refiners(&self) -> Vec<Arc<dyn AnnotationRefiner>> {
        Vec::new()
    }

    fn prompt_resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        Ok(vec![SkillResource {
            name: request
                .resource_name
                .clone()
                .unwrap_or_else(|| "summary".to_owned()),
            media_type: "text/markdown".to_owned(),
            content: "Return one allowed classification label.".to_owned(),
        }])
    }

    fn correction_taxonomy(&self) -> Vec<CorrectionKind> {
        Vec::new()
    }

    fn review_policy(&self) -> Arc<dyn ReviewPolicy> {
        Arc::new(DummyReview)
    }
}

#[tokio::test]
async fn external_skill_registers_and_executes_without_runtime_changes() {
    let mut skills = SkillRegistry::new();
    skills
        .register(Arc::new(DummySkill::new()))
        .expect("dummy skill registers");
    let dummy = skills.get("dummy").expect("dummy skill resolves");
    assert_eq!(
        dummy
            .workflow()
            .topological_order()
            .expect("valid workflow"),
        vec![TaskId::from("mood")]
    );

    let mut tools = ToolRegistry::new();
    for tool in dummy.tool_factories() {
        tools.register(tool).expect("dummy tool registers");
    }
    let temporary = tempfile::tempdir().expect("temporary project root");
    let context = ToolContext {
        project_root: temporary.path().to_path_buf(),
        run_id: annotagent_core::RunId::new(),
        image_id: None,
        image: None,
        task_id: Some(TaskId::from("mood")),
        cancellation: CancellationToken::new(),
    };
    let result = tools
        .execute(
            "submit_dummy_classification",
            &context,
            json!({"label": "bright"}),
        )
        .await
        .expect("dummy classification executes");
    assert_eq!(result.model_result["label"], "bright");
}

#[test]
fn two_skills_are_namespaced_and_visual_merge_is_deterministic() {
    let mut skills = SkillRegistry::new();
    skills
        .register(Arc::new(DummySkill::named(
            "zeta",
            json!({"color": "#ff0000"}),
        )))
        .expect("zeta Skill");
    skills
        .register(Arc::new(DummySkill::named(
            "alpha",
            json!({"color": "#00ff00"}),
        )))
        .expect("alpha Skill with same extension ids");
    let enabled = vec!["zeta".to_owned(), "alpha".to_owned()];
    let catalog = skills
        .validation_catalog_for(&enabled)
        .expect("namespaced catalog");
    assert_eq!(
        catalog.validators,
        ["alpha.check".to_owned(), "zeta.check".to_owned()]
            .into_iter()
            .collect()
    );
    let extensions = skills
        .namespaced_extensions_for(&enabled)
        .expect("namespaced extensions");
    assert_eq!(
        extensions.nodes,
        vec!["alpha.mood".to_owned(), "zeta.mood".to_owned()]
    );
    assert_eq!(
        extensions.tools,
        vec![
            "alpha.submit_dummy_classification".to_owned(),
            "zeta.submit_dummy_classification".to_owned(),
        ]
    );

    let merged = skills
        .merge_visual_profiles(&enabled)
        .expect("merged visual profile");
    assert_eq!(merged.sources["target"], "alpha");
    assert_eq!(merged.values["target"]["color"], "#00ff00");
    assert_eq!(merged.conflicts.len(), 1);
    assert_eq!(merged.conflicts[0].ignored_skill, "zeta");

    let project = annotagent_core::ProjectSchema::from_yaml(
        r#"
version: 1
project:
  name: Multi Skill
  enabled_skills:
    - { id: alpha, version: "1" }
    - { id: zeta, version: "1" }
dataset: { root: images }
runtime: {}
tasks: []
review: { auto_accept_confidence: 0.9, force_review_below: 0.5 }
export: {}
"#,
    )
    .expect("multi-Skill project schema");
    assert!(project.validate(&catalog).is_empty());
    assert_eq!(project.project.enabled_skill_versions().len(), 2);
}
