# HTTP Vision Detection Worker Protocol v1

This protocol connects AnnotAgent's Rust Runtime to optional, untrusted detection Workers. Core
sees only normalized typed Artifacts; Worker implementation language, model architecture, raw
coordinate format, and accelerator remain outside Core.

The JSON `protocol_version` is the integer `1`. A Worker exposes:

```text
GET  /health
GET  /v1/capabilities
POST /v1/infer
POST /v1/cancel
```

## Health

```json
{
  "protocol_version": 1,
  "worker_id": "local-detector",
  "model_id": "detector-v1",
  "status": "healthy",
  "detail": "ready"
}
```

`status` is `healthy`, `degraded`, `unavailable`, or `unknown`. An unavailable Worker does not
prevent AnnotAgent from starting; it remains visible as an unavailable model binding.

## Capability discovery

```json
{
  "protocol_version": 1,
  "worker_id": "local-detector",
  "model_id": "detector-v1",
  "capabilities": ["object_detection"],
  "score_semantics": "relative_confidence",
  "supports_visual_prompt": false,
  "supports_batch": false,
  "label_space": ["object"],
  "limits": {
    "max_images": 1,
    "max_input_artifacts": 0,
    "max_request_bytes": 20000000,
    "timeout_seconds": 30
  }
}
```

Capabilities are Worker-reported and checked against the registered Model Descriptor. Supported
detection capabilities are `object_detection`, `open_vocabulary_detection`, and
`phrase_grounding`. A capability or model identity mismatch is a protocol error.

## Inference

Requests carry one bounded inline PNG or JPEG. There is deliberately no filesystem path field.

```json
{
  "protocol_version": 1,
  "request_id": "request-uuid",
  "operation": "object_detection",
  "model_id": "detector-v1",
  "image": {
    "id": "image-uuid",
    "mime_type": "image/png",
    "data_base64": "..."
  },
  "queries": [],
  "target_labels": ["object"],
  "options": {
    "confidence_threshold": 0.25,
    "iou_threshold": 0.7,
    "max_detections": 100,
    "generation_mode": null
  },
  "timeout_ms": 30000
}
```

Open-vocabulary and phrase-grounding requests use bounded queries:

```json
{
  "id": "query-object",
  "text": "the target object description",
  "target_label": "object"
}
```

Workers return normalized xyxy geometry. Conversion from native model coordinates happens inside
the Worker adapter, and conversion from protocol xyxy to AnnotAgent's normalized xywh rectangle
happens inside the Rust Provider adapter.

```json
{
  "protocol_version": 1,
  "request_id": "request-uuid",
  "model_id": "detector-v1",
  "detections": [
    {
      "detection_id": "detection-1",
      "query_id": null,
      "model_label": "object",
      "target_label": "object",
      "bbox_xyxy_normalized": [0.2, 0.3, 0.5, 0.7],
      "score": 0.87,
      "score_semantics": "relative_confidence"
    }
  ],
  "usage": {"duration_ms": 24, "device": "cuda"},
  "warnings": [],
  "error": null
}
```

An explicit no-object result is `"detections": []` and is successful. A Worker that provides no
score sends `"score": null` with `"score_semantics": "not_provided"`; AnnotAgent never inserts a
default confidence.

Errors use the same response envelope and a structured error:

```json
{
  "code": "model_unavailable",
  "message": "model is not loaded",
  "retryable": false
}
```

## Cancellation

```json
{
  "protocol_version": 1,
  "request_id": "request-uuid",
  "model_id": "detector-v1"
}
```

The response repeats the protocol and request ID and returns `"cancelled": true` or `false`.
Runtime cancellation aborts the in-flight HTTP request and sends this cancellation request on a
bounded best-effort path.

## Trust boundary

- Loopback (`127.0.0.0/8`, `::1`, or `localhost`) is allowed by default. Remote Workers require
  explicit opt-in and HTTPS.
- URL credentials, query strings, fragments, non-HTTP schemes, and redirects are rejected.
- Authorization values are validated and never included in errors or traces. Redirects are
  disabled so headers cannot cross origins.
- Request bytes, decoded image bytes, query count/text, retry count, timeout, detection count,
  warning size, and response bytes are bounded.
- Responses reject unknown fields, protocol/model/request/capability mismatch, duplicate IDs,
  undeclared query/Project labels, invalid or non-finite scores, malformed JSON, and non-normalized
  or reversed geometry.
- Raw JSON is represented in evidence only by a controlled ID, media type, SHA-256, and byte count;
  full payloads, images, base64, sensitive headers, and arbitrary Worker paths are not logged.
