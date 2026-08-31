# AnnotAgent Vision Worker SDK

This package implements the versioned, model-brand-neutral HTTP boundary for expert vision
workers. It provides strict Pydantic contracts, FastAPI endpoint helpers, bounded image decoding,
coordinate validation, cancellation, Artifact serialization, conformance checks and worker
scaffolding.

Install for development without downloading model weights:

```bash
python -m pip install -e './sdk/python[test]'
python -m pytest sdk/python/tests
```

The SDK never downloads a checkpoint. A generated Worker reports `missing_weights` until its
developer explicitly supplies and validates a local model identity.
