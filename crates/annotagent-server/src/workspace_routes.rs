//! Project-owned data, execution, and Review route registration.
//!
//! Handlers remain in the parent during the incremental extraction. Keeping the complete route
//! family here makes ownership boundaries reviewable without a high-risk handler rewrite.

use axum::{
    Router,
    routing::{delete, get, patch, post},
};

use super::{
    ServerState, accept_project_review_and_next, accept_review_and_next, add_project_label,
    add_project_task, annotation_revisions, cancel_batch, cancel_run, create_annotation,
    create_project_geometry_calibration, export_dataset, get_batch, get_export_readiness,
    get_next_project_review, get_next_review, get_project, get_project_geometry_policy,
    get_project_guidance, get_project_readiness, get_project_review, get_project_summary,
    get_review, get_run, get_run_debug_summary, get_run_geometry_quality, get_run_result_summary,
    get_workflow_catalog, image_content, import_annotations, import_images,
    inspect_run_pipeline_artifacts, list_batches, list_images, list_project_agent_sessions,
    list_project_correction_memory, list_project_geometry_calibrations,
    list_project_geometry_corrections, list_project_reviews, list_reviews, list_run_annotations,
    list_run_reviews, list_run_summaries, patch_annotation, pause_batch, pause_run,
    project_review_revisions, put_project_geometry_policy, reject_project_review_and_next,
    reject_review_and_next, remove_image, replay_run_from_node, resume_batch, resume_run,
    review_decision, run_events, set_project_skills, start_batch, start_run,
};

pub(super) fn routes() -> Router<ServerState> {
    Router::new()
        .route("/api/runs", get(list_run_summaries))
        .route("/api/projects/{project_id}", get(get_project))
        .route(
            "/api/projects/{project_id}/guidance",
            get(get_project_guidance),
        )
        .route(
            "/api/projects/{project_id}/readiness",
            get(get_project_readiness),
        )
        .route(
            "/api/projects/{project_id}/summary",
            get(get_project_summary),
        )
        .route(
            "/api/projects/{project_id}/schema/labels",
            post(add_project_label),
        )
        .route(
            "/api/projects/{project_id}/schema/tasks",
            post(add_project_task),
        )
        .route(
            "/api/projects/{project_id}/skills",
            post(set_project_skills),
        )
        .route(
            "/api/projects/{project_id}/workflow-catalog",
            get(get_workflow_catalog),
        )
        .route("/api/projects/{project_id}/import", post(import_images))
        .route(
            "/api/projects/{project_id}/annotation-import",
            post(import_annotations),
        )
        .route("/api/projects/{project_id}/images", get(list_images))
        .route(
            "/api/projects/{project_id}/images/{index}",
            delete(remove_image),
        )
        .route(
            "/api/projects/{project_id}/agent-sessions",
            get(list_project_agent_sessions),
        )
        .route(
            "/api/projects/{project_id}/correction-memory",
            get(list_project_correction_memory),
        )
        .route(
            "/api/projects/{project_id}/geometry-corrections",
            get(list_project_geometry_corrections),
        )
        .route(
            "/api/projects/{project_id}/geometry-policy",
            get(get_project_geometry_policy).put(put_project_geometry_policy),
        )
        .route(
            "/api/projects/{project_id}/geometry-calibrations",
            get(list_project_geometry_calibrations).post(create_project_geometry_calibration),
        )
        .route(
            "/api/projects/{project_id}/images/{index}/content",
            get(image_content),
        )
        .route("/api/projects/{project_id}/runs", post(start_run))
        .route("/api/projects/{project_id}/batches", post(start_batch))
        .route("/api/batches", get(list_batches))
        .route("/api/batches/{batch_id}", get(get_batch))
        .route("/api/batches/{batch_id}/pause", post(pause_batch))
        .route("/api/batches/{batch_id}/resume", post(resume_batch))
        .route("/api/batches/{batch_id}/cancel", post(cancel_batch))
        .route(
            "/api/projects/{project_id}/export-readiness",
            get(get_export_readiness),
        )
        .route("/api/projects/{project_id}/export", post(export_dataset))
        .route("/api/runs/{run_id}", get(get_run))
        .route(
            "/api/runs/{run_id}/result-summary",
            get(get_run_result_summary),
        )
        .route(
            "/api/runs/{run_id}/debug-summary",
            get(get_run_debug_summary),
        )
        .route(
            "/api/runs/{run_id}/geometry-quality",
            get(get_run_geometry_quality),
        )
        .route(
            "/api/runs/{run_id}/pipeline-artifacts",
            get(inspect_run_pipeline_artifacts),
        )
        .route(
            "/api/runs/{run_id}/replay/{node_id}",
            post(replay_run_from_node),
        )
        .route("/api/runs/{run_id}/pause", post(pause_run))
        .route("/api/runs/{run_id}/resume", post(resume_run))
        .route("/api/runs/{run_id}/cancel", post(cancel_run))
        .route("/api/runs/{run_id}/events", get(run_events))
        .route(
            "/api/runs/{run_id}/annotations",
            get(list_run_annotations).post(create_annotation),
        )
        .route("/api/runs/{run_id}/reviews", get(list_run_reviews))
        .route(
            "/api/projects/{project_id}/reviews",
            get(list_project_reviews),
        )
        .route(
            "/api/projects/{project_id}/reviews/{review_id}",
            get(get_project_review),
        )
        .route(
            "/api/projects/{project_id}/reviews/{review_id}/next",
            get(get_next_project_review),
        )
        .route(
            "/api/projects/{project_id}/reviews/{review_id}/accept-and-next",
            post(accept_project_review_and_next),
        )
        .route(
            "/api/projects/{project_id}/reviews/{review_id}/reject-and-next",
            post(reject_project_review_and_next),
        )
        .route(
            "/api/projects/{project_id}/reviews/{review_id}/revisions",
            get(project_review_revisions),
        )
        .route("/api/reviews", get(list_reviews))
        .route("/api/reviews/{review_id}", get(get_review))
        .route("/api/reviews/{review_id}/next", get(get_next_review))
        .route("/api/reviews/{review_id}/decision", post(review_decision))
        .route(
            "/api/reviews/{review_id}/accept-and-next",
            post(accept_review_and_next),
        )
        .route(
            "/api/reviews/{review_id}/reject-and-next",
            post(reject_review_and_next),
        )
        .route("/api/annotations/{annotation_id}", patch(patch_annotation))
        .route(
            "/api/annotations/{annotation_id}/revisions",
            get(annotation_revisions),
        )
}
