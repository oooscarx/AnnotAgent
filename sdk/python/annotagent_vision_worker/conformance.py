"""Reusable black-box conformance checks for FastAPI Worker applications."""

from __future__ import annotations

from fastapi import FastAPI
from .models import ContractsResponse, ModelsResponse, PROTOCOL_VERSION


def assert_app_conformance(app: FastAPI) -> None:
    from fastapi.testclient import TestClient

    client = TestClient(app)
    health = client.get("/health")
    if health.status_code not in (200, 503):
        raise AssertionError(f"unexpected /health status {health.status_code}")
    capabilities = client.get("/v1/capabilities")
    if capabilities.status_code != 200 or capabilities.json()["protocol_version"] != PROTOCOL_VERSION:
        raise AssertionError("/v1/capabilities is not protocol v1")
    models = ModelsResponse.model_validate(client.get("/v1/models").json())
    contracts = ContractsResponse.model_validate(client.get("/v1/contracts").json())
    if models.worker_id != contracts.worker_id:
        raise AssertionError("model and contract worker identities differ")
    if {model.model_id for model in models.models} != {
        model.model_id for model in contracts.models
    }:
        raise AssertionError("model and contract discovery identities differ")
    unknown = client.post(
        "/v1/warmup",
        json={"protocol_version": 1, "request_id": "conformance", "model_id": "unknown"},
    )
    if unknown.status_code != 404:
        raise AssertionError("unknown model warmup must fail closed")
