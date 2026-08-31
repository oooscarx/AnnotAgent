"""Strict wire models for AnnotAgent HTTP Vision Worker Protocol v1."""

from __future__ import annotations

from datetime import datetime
from enum import StrEnum
from typing import Any, Annotated, Literal
from uuid import UUID

from pydantic import BaseModel, ConfigDict, Field, RootModel, field_validator, model_validator

PROTOCOL_VERSION = 1
MAX_ID_BYTES = 512
MAX_ARTIFACTS = 10_000
MAX_WARNINGS = 100


class StrictModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class Capability(StrEnum):
    VISION_LANGUAGE = "vision_language"
    IMAGE_CLASSIFICATION = "image_classification"
    OBJECT_DETECTION = "object_detection"
    OPEN_VOCABULARY_DETECTION = "open_vocabulary_detection"
    PHRASE_GROUNDING = "phrase_grounding"
    SEMANTIC_SEGMENTATION = "semantic_segmentation"
    PROMPTED_SEGMENTATION = "prompted_segmentation"
    INSTANCE_SEGMENTATION = "instance_segmentation"
    KEYPOINT_DETECTION = "keypoint_detection"


class ArtifactKind(StrEnum):
    IMAGE = "image"
    DETECTION_SET = "detection_set"
    BOX_PROMPT_SET = "box_prompt_set"
    POINT_PROMPT_SET = "point_prompt_set"
    MASK_SET = "mask_set"
    POLYGON_SET = "polygon_set"
    CANDIDATE_CLUSTER_SET = "candidate_cluster_set"
    CROP_SET = "crop_set"
    CLASSIFICATION_SET = "classification_set"
    ANNOTATION_CANDIDATE_SET = "annotation_candidate_set"
    CLASSIFICATION = "classification"
    BOUNDING_BOX = "bounding_box"
    KEYPOINTS = "keypoints"
    POLYLINE = "polyline"
    POLYGON = "polygon"
    SEMANTIC_MASK = "semantic_mask"
    INSTANCE_MASK = "instance_mask"
    ATTRIBUTES = "attributes"
    RELATIONS = "relations"


class ScoreSemantics(StrEnum):
    CALIBRATED_PROBABILITY = "calibrated_probability"
    RELATIVE_CONFIDENCE = "relative_confidence"
    RANKING_SCORE = "ranking_score"
    NOT_PROVIDED = "not_provided"
    UNKNOWN = "unknown"


class GeometrySemantics(StrEnum):
    NOT_APPLICABLE = "not_applicable"
    COARSE_HYPOTHESIS = "coarse_hypothesis"
    PREDICTED_GEOMETRY = "predicted_geometry"
    MASK_REFINED_GEOMETRY = "mask_refined_geometry"
    CALIBRATED_GEOMETRY = "calibrated_geometry"
    HUMAN_VERIFIED = "human_verified"


class ModelAvailability(StrEnum):
    UNCONFIGURED = "unconfigured"
    MISSING_WEIGHTS = "missing_weights"
    DISABLED = "disabled"
    UNKNOWN = "unknown"
    AVAILABLE = "available"
    UNREACHABLE = "unreachable"
    INCOMPATIBLE_PROTOCOL = "incompatible_protocol"
    INVALID_CONTRACT = "invalid_contract"
    FAILED_SMOKE_TEST = "failed_smoke_test"


class PromptKind(StrEnum):
    TEXT = "text"
    BOX = "box"
    POINT = "point"
    EXISTING_ANNOTATION = "existing_annotation"


class ContractDataType(RootModel[str | dict[str, ArtifactKind]]):
    @model_validator(mode="after")
    def validate_contract_type(self) -> "ContractDataType":
        if self.root == "text":
            return self
        if not isinstance(self.root, dict) or set(self.root) != {"artifact"}:
            raise ValueError("contract data type must be text or one artifact kind")
        return self


class ArtifactContract(StrictModel):
    name: str = Field(min_length=1, max_length=120)
    data_type: ContractDataType
    required: bool
    multiple: bool


class PromptContract(StrictModel):
    kind: PromptKind
    required: bool
    multiple: bool


class ProviderConnection(StrictModel):
    kind: Literal["provider_model"]
    provider_id: UUID
    remote_model_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)


class WorkerConnection(StrictModel):
    kind: Literal["vision_worker_model"]
    worker_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    worker_model_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)


class MockConnection(StrictModel):
    kind: Literal["mock"]
    fixture_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)


ModelConnection = Annotated[
    ProviderConnection | WorkerConnection | MockConnection,
    Field(discriminator="kind"),
]


class CheckpointIdentity(StrictModel):
    sha256: str = Field(pattern=r"^[0-9a-fA-F]{64}$")
    source: str | None = Field(default=None, max_length=MAX_ID_BYTES)
    training_dataset_version: str | None = Field(default=None, max_length=120)


class RuntimeRequirements(StrictModel):
    devices: list[str] = Field(default_factory=list, max_length=16)
    minimum_gpu_memory_mb: int | None = Field(default=None, gt=0)
    dependencies: list[str] = Field(default_factory=list, max_length=100)
    supports_batch: bool = False


class LicenseMetadata(StrictModel):
    code_license: str | None = Field(default=None, max_length=256)
    weight_license: str | None = Field(default=None, max_length=256)
    source_url: str | None = Field(default=None, max_length=2048)
    commercial_use: Literal["allowed", "restricted", "unknown"] = "unknown"
    redistribution: Literal["allowed", "restricted", "unknown"] = "unknown"
    usage_notes: list[str] = Field(default_factory=list, max_length=100)
    verified_from_official_source: bool = False


class AvailabilityEvidence(StrictModel):
    health_passed: bool = False
    protocol_compatible: bool = False
    contracts_validated: bool = False
    sample_conversion_passed: bool = False
    weights_ready: bool = False
    checked_at: datetime | None = None
    detail: str | None = Field(default=None, max_length=1000)


class ExpertModelManifest(StrictModel):
    schema_version: Literal["1"] = "1"
    model_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    display_name: str = Field(min_length=1, max_length=120)
    architecture: str | None = Field(default=None, max_length=120)
    model_version: str = Field(min_length=1, max_length=120)
    connection: ModelConnection
    capabilities: set[Capability] = Field(min_length=1)
    input_contracts: list[ArtifactContract] = Field(min_length=1, max_length=32)
    output_contracts: list[ArtifactContract] = Field(min_length=1, max_length=32)
    prompt_contracts: list[PromptContract] = Field(default_factory=list, max_length=16)
    score_semantics: ScoreSemantics = ScoreSemantics.UNKNOWN
    geometry_semantics: GeometrySemantics = GeometrySemantics.NOT_APPLICABLE
    label_space: list[str] | None = Field(default=None, max_length=100_000)
    checkpoint: CheckpointIdentity | None = None
    runtime_requirements: RuntimeRequirements = Field(default_factory=RuntimeRequirements)
    license: LicenseMetadata = Field(default_factory=LicenseMetadata)
    availability: ModelAvailability = ModelAvailability.UNKNOWN
    availability_evidence: AvailabilityEvidence = Field(default_factory=AvailabilityEvidence)
    metadata: dict[str, Any] = Field(default_factory=dict)

    @model_validator(mode="after")
    def validate_semantics_and_availability(self) -> "ExpertModelManifest":
        prompted = Capability.PROMPTED_SEGMENTATION in self.capabilities
        prompt_kinds = {contract.kind for contract in self.prompt_contracts}
        if prompted and not ({PromptKind.BOX, PromptKind.POINT} & prompt_kinds):
            raise ValueError("prompted segmentation requires box or point prompts")
        if prompted and self.geometry_semantics != GeometrySemantics.MASK_REFINED_GEOMETRY:
            raise ValueError("prompted segmentation requires mask-refined geometry semantics")
        input_kinds = {
            contract.data_type.root["artifact"]
            for contract in self.input_contracts
            if isinstance(contract.data_type.root, dict)
        }
        output_kinds = {
            contract.data_type.root["artifact"]
            for contract in self.output_contracts
            if isinstance(contract.data_type.root, dict)
        }
        if prompted and (
            ArtifactKind.IMAGE not in input_kinds
            or not ({ArtifactKind.BOX_PROMPT_SET, ArtifactKind.POINT_PROMPT_SET} & input_kinds)
            or ArtifactKind.MASK_SET not in output_kinds
        ):
            raise ValueError(
                "prompted segmentation requires image plus box/point prompt inputs and mask-set output"
            )
        if self.availability == ModelAvailability.AVAILABLE and not all(
            (
                self.availability_evidence.health_passed,
                self.availability_evidence.protocol_compatible,
                self.availability_evidence.contracts_validated,
                self.availability_evidence.sample_conversion_passed,
                self.availability_evidence.weights_ready,
            )
        ):
            raise ValueError("available models require complete registration evidence")
        if self.label_space is not None and (
            any(not label.strip() for label in self.label_space)
            or len(set(self.label_space)) != len(self.label_space)
        ):
            raise ValueError("label space must contain unique non-empty labels")
        return self


class WorkerModelSummary(StrictModel):
    model_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    display_name: str = Field(min_length=1, max_length=120)
    architecture: str | None = Field(default=None, max_length=120)
    model_version: str = Field(min_length=1, max_length=120)
    checkpoint_sha256: str | None = Field(default=None, pattern=r"^[0-9a-fA-F]{64}$")
    capabilities: list[str] = Field(min_length=1, max_length=32)
    availability: ModelAvailability


class VisionModelLimits(StrictModel):
    max_images: int | None = Field(default=None, gt=0)
    max_input_artifacts: int | None = Field(default=None, ge=0)
    max_request_bytes: int | None = Field(default=None, gt=0)
    timeout_seconds: int | None = Field(default=None, gt=0)


class HealthResponse(StrictModel):
    status: Literal["healthy", "degraded", "unavailable", "unknown"]
    detail: str | None = Field(default=None, max_length=1000)
    checked_at: datetime | None = None


class CapabilityResponse(StrictModel):
    protocol_version: Literal[1] = 1
    worker_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    model_identity: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    capabilities: list[str] = Field(min_length=1, max_length=32)
    input_types: list[Any] = Field(min_length=1, max_length=32)
    output_types: list[ArtifactKind] = Field(min_length=1, max_length=32)
    limits: VisionModelLimits = Field(default_factory=VisionModelLimits)
    models: list[WorkerModelSummary] = Field(default_factory=list, max_length=256)


class ModelsResponse(StrictModel):
    protocol_version: Literal[1] = 1
    worker_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    models: list[WorkerModelSummary] = Field(min_length=1, max_length=256)


class ContractsResponse(StrictModel):
    protocol_version: Literal[1] = 1
    worker_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    models: list[ExpertModelManifest] = Field(min_length=1, max_length=256)


class ModelImage(StrictModel):
    id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    mime_type: Literal["image/jpeg", "image/png"]
    data_base64: str = Field(min_length=1)


class InferenceRequest(StrictModel):
    protocol_version: Literal[1] = 1
    request_id: str = Field(min_length=1, max_length=128)
    operation: str = Field(min_length=1, max_length=120)
    run_id: UUID
    image_id: UUID
    task_id: str | None = Field(default=None, min_length=1, max_length=MAX_ID_BYTES)
    node_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    model_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    image: ModelImage | None = None
    input_artifacts: list[dict[str, Any]] = Field(default_factory=list, max_length=MAX_ARTIFACTS)
    prompt: str | None = Field(default=None, max_length=20_000)
    parameters: dict[str, Any] = Field(default_factory=dict)
    timeout_ms: int | None = Field(default=None, gt=0, le=600_000)
    cancellation_requested: bool = False


class WorkerError(StrictModel):
    code: str = Field(min_length=1, max_length=120)
    message: str = Field(min_length=1, max_length=1000)
    retryable: bool = False


class BackendUsage(StrictModel):
    source: str | None = Field(default=None, max_length=120)
    compute_milliseconds: int | None = Field(default=None, ge=0)
    input_megapixels: int | None = Field(default=None, ge=0)


class BackendTimings(StrictModel):
    queue_ms: int | None = Field(default=None, ge=0)
    preprocess_ms: int | None = Field(default=None, ge=0)
    inference_ms: int | None = Field(default=None, ge=0)
    postprocess_ms: int | None = Field(default=None, ge=0)
    total_ms: int | None = Field(default=None, ge=0)


class InferenceResponse(StrictModel):
    protocol_version: Literal[1] = 1
    model_identity: str | None = Field(default=None, max_length=MAX_ID_BYTES)
    artifacts: list[dict[str, Any]] = Field(default_factory=list, max_length=MAX_ARTIFACTS)
    request_id: str | None = Field(default=None, max_length=128)
    metadata: dict[str, Any] = Field(default_factory=dict)
    usage: BackendUsage = Field(default_factory=BackendUsage)
    warnings: list[str] = Field(default_factory=list, max_length=MAX_WARNINGS)
    timings: BackendTimings = Field(default_factory=BackendTimings)
    error: WorkerError | None = None

    @field_validator("warnings")
    @classmethod
    def bounded_warnings(cls, warnings: list[str]) -> list[str]:
        if any(not warning.strip() or len(warning) > 1000 for warning in warnings):
            raise ValueError("warnings must be non-empty and bounded")
        return warnings


class CancelRequest(StrictModel):
    protocol_version: Literal[1] = 1
    request_id: str = Field(min_length=1, max_length=128)
    model_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)


class CancelResponse(StrictModel):
    protocol_version: Literal[1] = 1
    request_id: str = Field(min_length=1, max_length=128)
    cancelled: bool


class WarmupRequest(StrictModel):
    protocol_version: Literal[1] = 1
    request_id: str = Field(min_length=1, max_length=128)
    model_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)


class WarmupResponse(StrictModel):
    protocol_version: Literal[1] = 1
    request_id: str = Field(min_length=1, max_length=128)
    model_id: str = Field(min_length=1, max_length=MAX_ID_BYTES)
    ready: bool
    duration_ms: int | None = Field(default=None, ge=0)
    error: WorkerError | None = None
