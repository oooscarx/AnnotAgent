use std::collections::{BTreeMap, BTreeSet};

use annotagent_model_bundle::{
    ModelBundleManifest, ModelContractDocument, ModelFileRole, ModelTensorContract, Sha256Digest,
};
use annotagent_model_runtime_onnx::{
    ExecutionProvider, ModelDescriptor, OnnxSession, SessionOptions, TensorDescriptor,
};
use annotagent_plugin_api::{PluginId, PluginManifest, PluginRuntimeStatus, PluginVersion};
use serde::{Deserialize, Serialize};

use crate::InstalledModelBundle;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginVersionRef {
    pub plugin_id: PluginId,
    pub plugin_version: PluginVersion,
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ModelBundleCompatibility {
    Compatible {
        plugin_versions: Vec<PluginVersionRef>,
    },
    MissingPlugin,
    IncompatiblePluginVersion,
    MissingFileRole {
        role: ModelFileRole,
    },
    ContractMismatch {
        detail: String,
    },
    UnsupportedFormat,
    UnsupportedPlatform,
    UnsupportedExecutionProvider,
    MissingLicenseAcceptance,
    PluginUnavailable,
}

pub struct ModelBundleCompatibilityResolver;

impl ModelBundleCompatibilityResolver {
    #[must_use]
    pub fn resolve(
        plugin: Option<&PluginManifest>,
        runtime_status: PluginRuntimeStatus,
        bundle: &ModelBundleManifest,
        model_id: &str,
        target: &str,
        execution_provider: &str,
        license_accepted: bool,
    ) -> ModelBundleCompatibility {
        let Some(plugin) = plugin else {
            return ModelBundleCompatibility::MissingPlugin;
        };
        if matches!(
            runtime_status,
            PluginRuntimeStatus::NotInstalled
                | PluginRuntimeStatus::Disabled
                | PluginRuntimeStatus::Crashed
                | PluginRuntimeStatus::Incompatible
        ) {
            return ModelBundleCompatibility::PluginUnavailable;
        }
        let Some(model) = plugin.models.iter().find(|model| model.id == model_id) else {
            return ModelBundleCompatibility::IncompatiblePluginVersion;
        };
        let Some(requirement) = bundle.compatible_plugins.iter().find(|requirement| {
            requirement.plugin_id == plugin.id && requirement.model_id == model_id
        }) else {
            return ModelBundleCompatibility::IncompatiblePluginVersion;
        };
        if !requirement.accepts(&plugin.id, &plugin.version, model_id) {
            return ModelBundleCompatibility::IncompatiblePluginVersion;
        }
        if !plugin
            .compatibility
            .targets
            .iter()
            .any(|item| item == target)
            || !bundle.runtime.platforms.contains(target)
        {
            return ModelBundleCompatibility::UnsupportedPlatform;
        }
        if !bundle
            .runtime
            .execution_providers
            .contains(execution_provider)
            || !model
                .runtime_requirements
                .devices
                .iter()
                .any(|device| device == execution_provider)
        {
            return ModelBundleCompatibility::UnsupportedExecutionProvider;
        }
        if bundle.format != annotagent_model_bundle::ModelFormat::Onnx
            || bundle.export.opset.is_none_or(|opset| opset > 21)
        {
            return ModelBundleCompatibility::UnsupportedFormat;
        }
        let provided = bundle
            .files
            .iter()
            .map(|file| file.role.clone())
            .collect::<BTreeSet<_>>();
        for role in &model.required_file_roles {
            let role =
                ModelFileRole::parse(role.clone()).expect("Plugin Manifest role was validated");
            if !provided.contains(&role) || !requirement.required_file_roles.contains(&role) {
                return ModelBundleCompatibility::MissingFileRole { role };
            }
        }
        if model.required_file_roles.len() != requirement.required_file_roles.len() {
            return ModelBundleCompatibility::ContractMismatch {
                detail: "Plugin and Bundle role sets differ".to_owned(),
            };
        }
        if !bundle
            .capabilities
            .iter()
            .all(|capability| model.capabilities.contains(capability))
        {
            return ModelBundleCompatibility::ContractMismatch {
                detail: "Bundle capability is not declared by the Plugin model".to_owned(),
            };
        }
        let model_hash = annotagent_plugin_api::Sha256Digest::of_bytes(
            &serde_json::to_vec(model).expect("Plugin model contract is serializable"),
        );
        if requirement.contract_hash.as_str() != model_hash.as_str() {
            return ModelBundleCompatibility::ContractMismatch {
                detail: "Plugin capability contract hash does not match the Bundle".to_owned(),
            };
        }
        if bundle.license.requires_acceptance && !license_accepted {
            return ModelBundleCompatibility::MissingLicenseAcceptance;
        }
        ModelBundleCompatibility::Compatible {
            plugin_versions: vec![PluginVersionRef {
                plugin_id: plugin.id.clone(),
                plugin_version: plugin.version.clone(),
                model_id: model_id.to_owned(),
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnnxRoleInspection {
    pub role: ModelFileRole,
    pub descriptor: ModelDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnnxContractInspection {
    pub contract_sha256: Sha256Digest,
    pub roles: BTreeMap<ModelFileRole, OnnxRoleInspection>,
    pub valid: bool,
    pub errors: Vec<String>,
}

#[must_use]
pub fn inspect_onnx_contract(
    bundle: &InstalledModelBundle,
    execution_provider: &str,
) -> OnnxContractInspection {
    let contract_reference = bundle.manifest.contracts.first();
    let contract_bytes = contract_reference
        .and_then(|reference| std::fs::read(bundle.content_root.join(&reference.path)).ok());
    let contract = contract_bytes
        .as_deref()
        .and_then(|bytes| ModelContractDocument::from_json(bytes).ok());
    let contract_sha256 = contract_reference.map_or_else(
        || Sha256Digest::of_bytes(b"missing-contract"),
        |reference| reference.sha256.clone(),
    );
    let mut inspection = OnnxContractInspection {
        contract_sha256,
        roles: BTreeMap::new(),
        valid: false,
        errors: Vec::new(),
    };
    let Some(contract) = contract else {
        inspection
            .errors
            .push("Model Contract cannot be parsed".to_owned());
        return inspection;
    };
    let provider = match execution_provider {
        "cpu" => ExecutionProvider::Cpu,
        "cuda" => ExecutionProvider::Cuda,
        "tensorrt" => ExecutionProvider::TensorRt,
        value => {
            inspection
                .errors
                .push(format!("Unsupported execution provider {value}"));
            return inspection;
        }
    };
    for file in &bundle.manifest.files {
        let Some(expected) = contract.roles.get(&file.role) else {
            inspection
                .errors
                .push(format!("Contract is missing role {}", file.role));
            continue;
        };
        match OnnxSession::load(
            bundle.content_root.join(&file.path),
            &SessionOptions {
                execution_provider: provider,
                ..SessionOptions::default()
            },
        ) {
            Ok(session) => {
                let descriptor = session.descriptor().clone();
                compare_tensors(
                    &file.role,
                    "input",
                    &expected.inputs,
                    &descriptor.inputs,
                    &mut inspection.errors,
                );
                compare_tensors(
                    &file.role,
                    "output",
                    &expected.outputs,
                    &descriptor.outputs,
                    &mut inspection.errors,
                );
                inspection.roles.insert(
                    file.role.clone(),
                    OnnxRoleInspection {
                        role: file.role.clone(),
                        descriptor,
                    },
                );
            }
            Err(error) => inspection
                .errors
                .push(format!("{} failed ONNX inspection: {error}", file.role)),
        }
    }
    for connection in &contract.connections {
        let source = inspection.roles.get(&connection.source_role);
        let target = inspection.roles.get(&connection.target_role);
        if source.is_none_or(|role| {
            !role
                .descriptor
                .outputs
                .iter()
                .any(|tensor| tensor.name == connection.source_output)
        }) || target.is_none_or(|role| {
            !role
                .descriptor
                .inputs
                .iter()
                .any(|tensor| tensor.name == connection.target_input)
        }) {
            inspection.errors.push(format!(
                "Connection {}:{} -> {}:{} is not present in ONNX descriptors",
                connection.source_role,
                connection.source_output,
                connection.target_role,
                connection.target_input
            ));
        }
    }
    inspection.valid =
        inspection.errors.is_empty() && inspection.roles.len() == bundle.manifest.files.len();
    inspection
}

fn compare_tensors(
    role: &ModelFileRole,
    kind: &str,
    expected: &[ModelTensorContract],
    actual: &[TensorDescriptor],
    errors: &mut Vec<String>,
) {
    if expected.len() != actual.len() {
        errors.push(format!(
            "Role {role} {kind} count mismatch: expected {}, found {}",
            expected.len(),
            actual.len()
        ));
        return;
    }
    for tensor in expected {
        let found = actual.iter().find(|candidate| {
            candidate.name == tensor.name || tensor.aliases.contains(&candidate.name)
        });
        let Some(found) = found else {
            errors.push(format!(
                "Role {role} is missing {kind} tensor {}",
                tensor.name
            ));
            continue;
        };
        if normalize_dtype(&found.element_type) != normalize_dtype(&tensor.dtype)
            || found.shape != tensor.shape
        {
            errors.push(format!(
                "Role {role} tensor {} expected {} {:?}, found {} {:?}",
                tensor.name, tensor.dtype, tensor.shape, found.element_type, found.shape
            ));
        }
    }
}

fn normalize_dtype(value: &str) -> &str {
    match value {
        "float32" | "f32" => "f32",
        "int64" | "i64" => "i64",
        "uint8" | "u8" => "u8",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs};

    use annotagent_core::ModelCapability;
    use annotagent_model_bundle::{
        CommercialUseStatus, ModelBundleFile, ModelBundleId, ModelBundleSignatureState,
        ModelBundleStatus, ModelContractReference, ModelExportMetadata, ModelFileRole, ModelFormat,
        ModelLicenseMetadata, ModelRuntimeMetadata, ModelSourceMetadata, ModelTestSuiteReference,
        PluginCompatibilityRequirement, RedistributionStatus,
    };
    use annotagent_plugin_api::{PluginManifest, PluginRuntimeStatus};
    use chrono::Utc;
    use prost::Message;
    use semver::Version;

    use crate::{InstalledModelBundle, ModelBundleInstallSource, ModelBundleVerification};

    use super::*;

    #[derive(Clone, PartialEq, Message)]
    struct ModelProto {
        #[prost(int64, tag = "1")]
        ir_version: i64,
        #[prost(string, tag = "2")]
        producer_name: String,
        #[prost(message, optional, tag = "7")]
        graph: Option<GraphProto>,
        #[prost(message, repeated, tag = "8")]
        opset_import: Vec<OperatorSetIdProto>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct OperatorSetIdProto {
        #[prost(string, tag = "1")]
        domain: String,
        #[prost(int64, tag = "2")]
        version: i64,
    }

    #[derive(Clone, PartialEq, Message)]
    struct GraphProto {
        #[prost(message, repeated, tag = "1")]
        node: Vec<NodeProto>,
        #[prost(string, tag = "2")]
        name: String,
        #[prost(message, repeated, tag = "11")]
        input: Vec<ValueInfoProto>,
        #[prost(message, repeated, tag = "12")]
        output: Vec<ValueInfoProto>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct NodeProto {
        #[prost(string, repeated, tag = "1")]
        input: Vec<String>,
        #[prost(string, repeated, tag = "2")]
        output: Vec<String>,
        #[prost(string, tag = "4")]
        op_type: String,
    }

    #[derive(Clone, PartialEq, Message)]
    struct ValueInfoProto {
        #[prost(string, tag = "1")]
        name: String,
        #[prost(message, optional, tag = "2")]
        r#type: Option<TypeProto>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct TypeProto {
        #[prost(message, optional, tag = "1")]
        tensor_type: Option<TypeProtoTensor>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct TypeProtoTensor {
        #[prost(int32, tag = "1")]
        elem_type: i32,
        #[prost(message, optional, tag = "2")]
        shape: Option<TensorShapeProto>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct TensorShapeProto {
        #[prost(message, repeated, tag = "1")]
        dim: Vec<TensorShapeDimension>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct TensorShapeDimension {
        #[prost(int64, tag = "1")]
        dim_value: i64,
    }

    #[test]
    fn real_onnx_metadata_must_match_the_declared_contract() {
        let temp = tempfile::tempdir().expect("temp");
        let model_path = temp.path().join("files/model.onnx");
        fs::create_dir_all(model_path.parent().expect("parent")).expect("dirs");
        fs::write(&model_path, identity_model()).expect("model");
        let contract_path = temp.path().join("contracts/model-contract.json");
        fs::create_dir_all(contract_path.parent().expect("parent")).expect("dirs");
        let contract = contract_json("input");
        fs::write(&contract_path, &contract).expect("contract");
        let bundle = installed_bundle(temp.path(), &model_path, &contract);
        let inspection = inspect_onnx_contract(&bundle, "cpu");
        assert!(inspection.valid, "{:?}", inspection.errors);
        assert_eq!(
            inspection.roles[&ModelFileRole::parse("model").expect("role")]
                .descriptor
                .inputs[0]
                .name,
            "input"
        );

        fs::write(&contract_path, contract_json("wrong_input")).expect("wrong contract");
        let invalid = inspect_onnx_contract(&bundle, "cpu");
        assert!(!invalid.valid);
        assert!(
            invalid
                .errors
                .iter()
                .any(|error| error.contains("wrong_input"))
        );
    }

    #[test]
    fn resolver_reports_license_and_role_failures_before_binding() {
        let plugin = PluginManifest::from_toml(include_str!(
            "../../../plugins/yolo-onnx/annotagent-plugin.toml"
        ))
        .expect("Plugin Manifest");
        let temp = tempfile::tempdir().expect("temp");
        let model_path = temp.path().join("model.onnx");
        fs::write(&model_path, identity_model()).expect("model");
        let contract = contract_json("input");
        let mut bundle = installed_bundle(temp.path(), &model_path, &contract).manifest;
        let plugin_model = &plugin.models[0];
        bundle.compatible_plugins = vec![PluginCompatibilityRequirement {
            plugin_id: plugin.id.clone(),
            plugin_version: ">=1.0.0,<2.0.0".to_owned(),
            model_id: plugin_model.id.clone(),
            contract_hash: Sha256Digest::of_bytes(
                &serde_json::to_vec(plugin_model).expect("contract"),
            ),
            required_file_roles: BTreeSet::from([ModelFileRole::parse("model").expect("role")]),
        }];
        bundle.runtime.platforms = BTreeSet::from([current_target()]);
        assert!(matches!(
            ModelBundleCompatibilityResolver::resolve(
                Some(&plugin),
                PluginRuntimeStatus::Installed,
                &bundle,
                &plugin_model.id,
                &current_target(),
                "cpu",
                false,
            ),
            ModelBundleCompatibility::MissingLicenseAcceptance
        ));
        assert!(matches!(
            ModelBundleCompatibilityResolver::resolve(
                Some(&plugin),
                PluginRuntimeStatus::Installed,
                &bundle,
                &plugin_model.id,
                &current_target(),
                "cpu",
                true,
            ),
            ModelBundleCompatibility::Compatible { .. }
        ));
        bundle.files[0].role = ModelFileRole::parse("other").expect("role");
        assert!(matches!(
            ModelBundleCompatibilityResolver::resolve(
                Some(&plugin),
                PluginRuntimeStatus::Installed,
                &bundle,
                &plugin_model.id,
                &current_target(),
                "cpu",
                true,
            ),
            ModelBundleCompatibility::MissingFileRole { .. }
        ));
    }

    fn identity_model() -> Vec<u8> {
        let value = |name: &str| ValueInfoProto {
            name: name.to_owned(),
            r#type: Some(TypeProto {
                tensor_type: Some(TypeProtoTensor {
                    elem_type: 1,
                    shape: Some(TensorShapeProto {
                        dim: vec![
                            TensorShapeDimension { dim_value: 1 },
                            TensorShapeDimension { dim_value: 2 },
                        ],
                    }),
                }),
            }),
        };
        ModelProto {
            ir_version: 8,
            producer_name: "annotagent-model-bundle-fixture".to_owned(),
            graph: Some(GraphProto {
                node: vec![NodeProto {
                    input: vec!["input".to_owned()],
                    output: vec!["output".to_owned()],
                    op_type: "Identity".to_owned(),
                }],
                name: "identity".to_owned(),
                input: vec![value("input")],
                output: vec![value("output")],
            }),
            opset_import: vec![OperatorSetIdProto {
                domain: String::new(),
                version: 13,
            }],
        }
        .encode_to_vec()
    }

    fn contract_json(input_name: &str) -> Vec<u8> {
        serde_json::to_vec_pretty(&serde_json::json!({
            "contract_version": "1",
            "roles": {
                "model": {
                    "inputs": [{"name": input_name, "aliases": [], "dtype": "float32", "shape": [1, 2]}],
                    "outputs": [{"name": "output", "aliases": [], "dtype": "float32", "shape": [1, 2]}]
                }
            },
            "connections": []
        }))
        .expect("contract")
    }

    fn installed_bundle(
        root: &std::path::Path,
        model_path: &std::path::Path,
        contract: &[u8],
    ) -> InstalledModelBundle {
        let role = ModelFileRole::parse("model").expect("role");
        let model_bytes = fs::read(model_path).expect("model");
        let manifest = ModelBundleManifest {
            schema_version: "1".to_owned(),
            id: ModelBundleId::parse("org.annotagent.models.identity-fixture").expect("id"),
            version: Version::new(1, 0, 0),
            display_name: "Identity Fixture".to_owned(),
            description: Some("Contract fixture".to_owned()),
            model_family: "fixture".to_owned(),
            architecture: "identity".to_owned(),
            format: ModelFormat::Onnx,
            variant: "tiny".to_owned(),
            capabilities: BTreeSet::from([ModelCapability::ObjectDetection]),
            compatible_plugins: Vec::new(),
            files: vec![ModelBundleFile {
                role: role.clone(),
                path: model_path
                    .strip_prefix(root)
                    .expect("relative")
                    .to_string_lossy()
                    .to_string(),
                sha256: Sha256Digest::of_bytes(&model_bytes),
                size_bytes: u64::try_from(model_bytes.len()).expect("size"),
                external_data_files: Vec::new(),
            }],
            contracts: vec![ModelContractReference {
                id: "model".to_owned(),
                path: "contracts/model-contract.json".to_owned(),
                sha256: Sha256Digest::of_bytes(contract),
                file_roles: BTreeSet::from([role]),
            }],
            transforms: Vec::new(),
            source: ModelSourceMetadata {
                upstream_project: "AnnotAgent".to_owned(),
                upstream_model_id: "identity".to_owned(),
                upstream_version: Some("1".to_owned()),
                upstream_checkpoint_sha256: None,
                source_url: None,
            },
            export: ModelExportMetadata {
                exporter_name: "fixture".to_owned(),
                exporter_version: "1".to_owned(),
                exporter_revision: None,
                export_date: None,
                opset: Some(13),
                numerical_validation: None,
            },
            runtime: ModelRuntimeMetadata {
                execution_providers: BTreeSet::from(["cpu".to_owned()]),
                platforms: BTreeSet::from([current_target()]),
                minimum_memory_mb: 64,
                recommended_memory_mb: 128,
            },
            license: ModelLicenseMetadata {
                name: "CC0-1.0".to_owned(),
                license_url: None,
                license_file: "licenses/MODEL-LICENSE".to_owned(),
                source_notice: None,
                license_digest: Sha256Digest::of_bytes(b"CC0"),
                redistribution: RedistributionStatus::Allowed,
                commercial_use: CommercialUseStatus::Allowed,
                requires_acceptance: true,
                usage_notes: Vec::new(),
            },
            test_suite: ModelTestSuiteReference {
                test_id: "identity".to_owned(),
                input_artifacts: vec!["tests/input.json".to_owned()],
                expected_summary: "tests/expected.json".to_owned(),
                tolerances: "tests/tolerances.json".to_owned(),
            },
            fixture: true,
            publishable: false,
        };
        InstalledModelBundle {
            bundle_digest: Sha256Digest::of_bytes(b"bundle"),
            manifest,
            status: ModelBundleStatus::Installed,
            source: ModelBundleInstallSource::LocalImport,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
            verification: ModelBundleVerification {
                verified_at: Utc::now(),
                bundle_digest: Sha256Digest::of_bytes(b"bundle"),
                signature: ModelBundleSignatureState::Unsigned,
                file_count: 2,
                manifest_valid: true,
                checksums_valid: true,
            },
            enabled: true,
            content_root: root.to_owned(),
        }
    }

    fn current_target() -> String {
        let os = match std::env::consts::OS {
            "macos" => "macos",
            "windows" => "windows",
            _ => "linux",
        };
        format!("{os}-{}", std::env::consts::ARCH)
    }
}
