#![forbid(unsafe_code)]

//! Reusable ONNX Runtime session management for official Rust expert-model plugins.

use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use annotagent_model_runtime_common::{TensorF32, validate_finite};
use ort::{
    execution_providers::{CPUExecutionProvider, CUDAExecutionProvider, TensorRTExecutionProvider},
    session::{Session, builder::GraphOptimizationLevel, input::SessionInputValue},
    tensor::TensorElementType,
    value::{Tensor, ValueType},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OnnxRuntimeError {
    #[error("failed to read model {path}: {source}")]
    ReadModel {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("ONNX Runtime error: {0}")]
    Runtime(#[from] ort::Error),
    #[error("session mutex was poisoned")]
    Poisoned,
    #[error("inference was cancelled at the {boundary} boundary")]
    Cancelled { boundary: &'static str },
    #[error("model expects {expected} inputs but received {actual}")]
    InputCount { expected: usize, actual: usize },
    #[error("unsupported tensor data type {0}")]
    UnsupportedDataType(String),
    #[error("tensor {name} has invalid shape: {reason}")]
    InvalidShape { name: String, reason: String },
    #[error("input {name} contains invalid numeric data: {reason}")]
    InvalidNumericData { name: String, reason: String },
    #[error("warmup iteration count must be at least one")]
    EmptyWarmup,
}

pub type Result<T> = std::result::Result<T, OnnxRuntimeError>;

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionProvider {
    #[default]
    Cpu,
    Cuda,
    TensorRt,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionOptions {
    #[serde(default)]
    pub execution_provider: ExecutionProvider,
    #[serde(default = "default_threads")]
    pub intra_threads: usize,
    #[serde(default = "default_threads")]
    pub inter_threads: usize,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            execution_provider: ExecutionProvider::Cpu,
            intra_threads: default_threads(),
            inter_threads: default_threads(),
        }
    }
}

const fn default_threads() -> usize {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorDescriptor {
    pub name: String,
    pub element_type: String,
    /// A negative value denotes a dynamic dimension.
    pub shape: Vec<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub sha256: String,
    pub inputs: Vec<TensorDescriptor>,
    pub outputs: Vec<TensorDescriptor>,
    pub execution_provider: ExecutionProvider,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "element_type", content = "values", rename_all = "snake_case")]
pub enum TensorData {
    Float32(Vec<f32>),
    Int64(Vec<i64>),
    Uint8(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedTensor {
    pub name: String,
    pub shape: Vec<usize>,
    pub data: TensorData,
}

impl NamedTensor {
    #[must_use]
    pub fn float32(name: impl Into<String>, tensor: TensorF32) -> Self {
        Self {
            name: name.into(),
            shape: tensor.shape,
            data: TensorData::Float32(tensor.values),
        }
    }

    fn validate(&self) -> Result<()> {
        let expected = self.shape.iter().try_fold(1_usize, |count, dimension| {
            if *dimension == 0 {
                None
            } else {
                count.checked_mul(*dimension)
            }
        });
        let actual = match &self.data {
            TensorData::Float32(values) => {
                validate_finite(values).map_err(|error| OnnxRuntimeError::InvalidNumericData {
                    name: self.name.clone(),
                    reason: error.to_string(),
                })?;
                values.len()
            }
            TensorData::Int64(values) => values.len(),
            TensorData::Uint8(values) => values.len(),
        };
        if expected != Some(actual) {
            return Err(OnnxRuntimeError::InvalidShape {
                name: self.name.clone(),
                reason: format!("shape requires {expected:?} values, received {actual}"),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InferenceOutputs {
    pub tensors: Vec<NamedTensor>,
    pub duration: Duration,
}

#[derive(Clone, Default, Debug)]
pub struct InferenceCancellation {
    cancelled: Arc<AtomicBool>,
}

impl InferenceCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn check(&self, boundary: &'static str) -> Result<()> {
        if self.is_cancelled() {
            Err(OnnxRuntimeError::Cancelled { boundary })
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub struct OnnxSession {
    descriptor: ModelDescriptor,
    session: Mutex<Session>,
}

impl OnnxSession {
    pub fn load(path: impl AsRef<Path>, options: &SessionOptions) -> Result<Self> {
        let path = path.as_ref();
        let sha256 = model_sha256(path)?;
        let mut builder = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(options.intra_threads.max(1))?
            .with_inter_threads(options.inter_threads.max(1))?;
        builder = match options.execution_provider {
            ExecutionProvider::Cpu => {
                builder.with_execution_providers([CPUExecutionProvider::default()
                    .build()
                    .error_on_failure()])?
            }
            ExecutionProvider::Cuda => builder.with_execution_providers([
                CUDAExecutionProvider::default().build().error_on_failure(),
                CPUExecutionProvider::default().build(),
            ])?,
            ExecutionProvider::TensorRt => builder.with_execution_providers([
                TensorRTExecutionProvider::default()
                    .build()
                    .error_on_failure(),
                CUDAExecutionProvider::default().build(),
                CPUExecutionProvider::default().build(),
            ])?,
        };
        let session = builder.commit_from_file(path)?;
        let inputs = session
            .inputs
            .iter()
            .map(|input| describe_tensor(&input.name, &input.input_type))
            .collect::<Result<Vec<_>>>()?;
        let outputs = session
            .outputs
            .iter()
            .map(|output| describe_tensor(&output.name, &output.output_type))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            descriptor: ModelDescriptor {
                sha256,
                inputs,
                outputs,
                execution_provider: options.execution_provider,
            },
            session: Mutex::new(session),
        })
    }

    #[must_use]
    pub fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }

    pub fn infer(
        &self,
        inputs: &[NamedTensor],
        cancellation: &InferenceCancellation,
    ) -> Result<InferenceOutputs> {
        cancellation.check("before_session")?;
        if inputs.len() != self.descriptor.inputs.len() {
            return Err(OnnxRuntimeError::InputCount {
                expected: self.descriptor.inputs.len(),
                actual: inputs.len(),
            });
        }
        for input in inputs {
            input.validate()?;
        }
        let values = inputs
            .iter()
            .map(named_tensor_to_input)
            .collect::<Result<Vec<_>>>()?;
        cancellation.check("before_run")?;
        let started = Instant::now();
        let mut session = self
            .session
            .lock()
            .map_err(|_| OnnxRuntimeError::Poisoned)?;
        let outputs = session.run(values)?;
        let duration = started.elapsed();
        let tensors = outputs
            .iter()
            .map(|(name, value)| output_to_named_tensor(name, &value))
            .collect::<Result<Vec<_>>>()?;
        drop(outputs);
        drop(session);
        cancellation.check("after_run")?;
        Ok(InferenceOutputs { tensors, duration })
    }

    pub fn warmup(
        &self,
        inputs: &[NamedTensor],
        iterations: usize,
        cancellation: &InferenceCancellation,
    ) -> Result<Duration> {
        if iterations == 0 {
            return Err(OnnxRuntimeError::EmptyWarmup);
        }
        let started = Instant::now();
        for _ in 0..iterations {
            self.infer(inputs, cancellation)?;
        }
        Ok(started.elapsed())
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct SessionCacheKey {
    sha256: String,
    options: SessionOptions,
}

#[derive(Default, Debug)]
pub struct SessionCache {
    sessions: Mutex<HashMap<SessionCacheKey, Arc<OnnxSession>>>,
}

impl SessionCache {
    pub fn get_or_load(
        &self,
        path: impl AsRef<Path>,
        options: &SessionOptions,
    ) -> Result<Arc<OnnxSession>> {
        let path = path.as_ref();
        let key = SessionCacheKey {
            sha256: model_sha256(path)?,
            options: options.clone(),
        };
        if let Some(session) = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::Poisoned)?
            .get(&key)
            .cloned()
        {
            return Ok(session);
        }
        let loaded = Arc::new(OnnxSession::load(path, options)?);
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::Poisoned)?;
        Ok(Arc::clone(
            sessions.entry(key).or_insert_with(|| Arc::clone(&loaded)),
        ))
    }

    pub fn evict_model(&self, sha256: &str) -> Result<usize> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::Poisoned)?;
        let previous = sessions.len();
        sessions.retain(|key, _| key.sha256 != sha256);
        Ok(previous - sessions.len())
    }

    pub fn len(&self) -> Result<usize> {
        Ok(self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::Poisoned)?
            .len())
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
}

pub fn model_sha256(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let mut file = File::open(path).map_err(|source| OnnxRuntimeError::ReadModel {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| OnnxRuntimeError::ReadModel {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn named_tensor_to_input(input: &NamedTensor) -> Result<(String, SessionInputValue<'static>)> {
    let shape = input.shape.clone();
    let value = match &input.data {
        TensorData::Float32(values) => {
            SessionInputValue::from(Tensor::from_array((shape, values.clone()))?)
        }
        TensorData::Int64(values) => {
            SessionInputValue::from(Tensor::from_array((shape, values.clone()))?)
        }
        TensorData::Uint8(values) => {
            SessionInputValue::from(Tensor::from_array((shape, values.clone()))?)
        }
    };
    Ok((input.name.clone(), value))
}

fn output_to_named_tensor(name: &str, value: &ort::value::DynValue) -> Result<NamedTensor> {
    match value.dtype() {
        ValueType::Tensor {
            ty: TensorElementType::Float32,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<f32>()?;
            Ok(NamedTensor {
                name: name.to_owned(),
                shape: shape.iter().map(|dimension| *dimension as usize).collect(),
                data: TensorData::Float32(values.to_vec()),
            })
        }
        ValueType::Tensor {
            ty: TensorElementType::Int64,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<i64>()?;
            Ok(NamedTensor {
                name: name.to_owned(),
                shape: shape.iter().map(|dimension| *dimension as usize).collect(),
                data: TensorData::Int64(values.to_vec()),
            })
        }
        ValueType::Tensor {
            ty: TensorElementType::Uint8,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<u8>()?;
            Ok(NamedTensor {
                name: name.to_owned(),
                shape: shape.iter().map(|dimension| *dimension as usize).collect(),
                data: TensorData::Uint8(values.to_vec()),
            })
        }
        other => Err(OnnxRuntimeError::UnsupportedDataType(format!("{other:?}"))),
    }
}

fn describe_tensor(name: &str, value_type: &ValueType) -> Result<TensorDescriptor> {
    match value_type {
        ValueType::Tensor { ty, shape, .. } => Ok(TensorDescriptor {
            name: name.to_owned(),
            element_type: ty.to_string(),
            shape: shape.iter().copied().collect(),
        }),
        other => Err(OnnxRuntimeError::UnsupportedDataType(format!("{other:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc};

    use prost::Message;
    use tempfile::tempdir;

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
        #[prost(string, tag = "3")]
        name: String,
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

    fn value_info(name: &str) -> ValueInfoProto {
        ValueInfoProto {
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
        }
    }

    fn write_identity_model(path: &Path) {
        let model = ModelProto {
            ir_version: 8,
            producer_name: "annotagent-rust-fixture".to_owned(),
            graph: Some(GraphProto {
                node: vec![NodeProto {
                    input: vec!["input".to_owned()],
                    output: vec!["output".to_owned()],
                    name: "identity".to_owned(),
                    op_type: "Identity".to_owned(),
                }],
                name: "tiny_identity".to_owned(),
                input: vec![value_info("input")],
                output: vec![value_info("output")],
            }),
            opset_import: vec![OperatorSetIdProto {
                domain: String::new(),
                version: 13,
            }],
        };
        fs::write(path, model.encode_to_vec()).expect("write model");
    }

    fn input() -> NamedTensor {
        NamedTensor {
            name: "input".to_owned(),
            shape: vec![1, 2],
            data: TensorData::Float32(vec![2.5, -4.0]),
        }
    }

    #[test]
    fn real_runtime_executes_tiny_identity_graph() {
        let directory = tempdir().expect("tempdir");
        let model_path = directory.path().join("identity.onnx");
        write_identity_model(&model_path);
        let session = OnnxSession::load(&model_path, &SessionOptions::default()).expect("load");
        assert_eq!(session.descriptor().inputs[0].shape, [1, 2]);
        assert_eq!(session.descriptor().inputs[0].element_type, "f32");
        let result = session
            .infer(&[input()], &InferenceCancellation::default())
            .expect("infer");
        assert_eq!(result.tensors[0].data, TensorData::Float32(vec![2.5, -4.0]));
        assert_eq!(result.tensors[0].shape, [1, 2]);
        assert_eq!(session.descriptor().sha256.len(), 64);
    }

    #[test]
    fn cache_keys_exact_model_and_options() {
        let directory = tempdir().expect("tempdir");
        let model_path = directory.path().join("identity.onnx");
        write_identity_model(&model_path);
        let cache = SessionCache::default();
        let first = cache
            .get_or_load(&model_path, &SessionOptions::default())
            .expect("first");
        let second = cache
            .get_or_load(&model_path, &SessionOptions::default())
            .expect("second");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(cache.len().expect("len"), 1);
        assert_eq!(
            cache
                .evict_model(&first.descriptor().sha256)
                .expect("evict"),
            1
        );
    }

    #[test]
    fn cancellation_is_checked_before_entering_runtime() {
        let directory = tempdir().expect("tempdir");
        let model_path = directory.path().join("identity.onnx");
        write_identity_model(&model_path);
        let session = OnnxSession::load(&model_path, &SessionOptions::default()).expect("load");
        let cancellation = InferenceCancellation::default();
        cancellation.cancel();
        assert!(matches!(
            session.infer(&[input()], &cancellation),
            Err(OnnxRuntimeError::Cancelled {
                boundary: "before_session"
            })
        ));
    }
}
