# Rust ONNX model runtime

AnnotAgent's official expert-model plugins share two model-neutral Rust crates:

- annotagent-model-runtime-common contains deterministic image resize and letterbox transforms,
  normalization, NCHW/NHWC tensor conversion, bounding-box conversion and clipping, class-aware
  NMS, mask threshold/resize, connected components, contour extraction, polygon simplification and
  finite-number validation.
- annotagent-model-runtime-onnx owns native ONNX model loading, input/output discovery, typed
  tensors, execution-provider selection, thread configuration, warmup, exact model SHA-256 and a
  process-local session cache.

Model-specific preprocessing, tensor names, output decoding, class mapping, prompt encoding,
palette handling and Artifact construction belong to each plugin. Neither shared crate contains a
branch for a particular model family.

## Execution providers

CPU is the portable default. CUDA and TensorRT can be selected explicitly through SessionOptions.
An explicitly requested accelerator is registered with error-on-failure; absence or initialization
failure is surfaced instead of silently describing a CPU session as accelerated. CPU remains last
in an accelerator provider list so supported graph operators may fall back after the requested
provider has initialized.

The Alpha validates CPU in continuous tests. CUDA and TensorRT are live-conditional on a compatible
native ONNX Runtime distribution, driver, accelerator libraries and hardware. Plugin manifests
must declare their supported targets and cannot become Ready until the selected configuration
passes its installed smoke test.

## Tensor and session contract

The runtime currently accepts and returns contiguous float32, int64 and uint8 tensors. Session
discovery exposes every tensor name, element type and shape; -1 denotes a dynamic dimension. Inputs
reject zero/overflowing shapes, value-count mismatches and non-finite floating point values before
entering the native runtime.

SessionCache keys a session by the model file's SHA-256 and the complete execution-provider and
thread configuration. The cache never aliases two checkpoint files or device configurations.
Plugins freeze the same model digest through their Registry weight set and Published Workflow
snapshot.

Warmup performs one or more real inference calls. Cancellation is checked before session entry,
immediately before the native call and after it returns. The current portable contract does not
preempt every operator in the middle of one native call; plugin process termination remains the
hard containment boundary for a stuck native runtime.

## Test fixture

The M4 test constructs a legal, weight-free ONNX Identity graph from the published protobuf wire
contract, writes it to an isolated temporary directory, loads it through the native CPU provider
and verifies that [2.5, -4.0] is returned unchanged. It also verifies graph input shape/type
discovery, SHA-256 identity, session cache reuse/eviction and cancellation before native entry.

This is genuine ONNX graph execution, but it is intentionally not advertised as an expert model or
registered in the product Model Registry. M5 supplies the first release-blocking expert vision
model plugin.
