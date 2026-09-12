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
    create_project_geometry_calibration, execute_project_management, export_dataset, get_batch,
    get_export_readiness, get_next_project_review, get_next_review, get_project,
    get_project_geometry_policy, get_project_guidance, get_project_management_operation,
    get_project_management_usage, get_project_readiness, get_project_review, get_project_summary,
    get_review, get_run, get_run_debug_summary, get_run_geometry_quality, get_run_provenance,
    get_run_result_summary, get_workflow_catalog, image_content, import_annotations, import_images,
    inspect_run_pipeline_artifacts, list_batches, list_images, list_project_agent_sessions,
    list_project_correction_memory, list_project_geometry_calibrations,
    list_project_geometry_corrections, list_project_pipeline_lifecycle, list_project_reviews,
    list_project_trash, list_reviews, list_run_annotations, list_run_reviews, list_run_summaries,
    patch_annotation, pause_batch, pause_run, preview_project_management, project_review_revisions,
    put_project_geometry_policy, reject_project_review_and_next, reject_review_and_next,
    remove_image, replay_run_from_node, resume_batch, resume_run, review_decision, run_events,
    set_project_skills, start_batch, start_run, upload_project_image,
};

pub(super) fn routes() -> Router<ServerState> {
    Router::new()
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-intent", get(super::task_delivery::get).post(super::task_delivery::save))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-schema", get(super::task_delivery::current_schema).post(super::task_delivery::prepare_schema))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/formal-result", get(super::task_delivery::formal_result))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-review-items", get(super::task_delivery::review_items))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-packages", post(super::training_delivery::start))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-package-consents", get(super::training_delivery::list_consents).post(super::training_delivery::authorize_consent))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-package-consents/{package_id}", get(super::training_delivery::get_consent))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-package-consents/{package_id}/cancel", post(super::training_delivery::cancel_consent))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-packages/{package_id}", get(super::training_delivery::status))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-packages/{package_id}/cancel", post(super::training_delivery::cancel))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-packages/{package_id}/download", get(super::training_delivery::download))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-images/{image_id}", get(super::task_delivery::image).post(super::task_delivery::confirm_image))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-images/{image_id}/missing-objects", post(super::task_delivery::create_object))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-images/{image_id}/objects", post(super::task_delivery::edit_object))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/delivery-images/{image_id}/preset-objects", post(super::task_delivery::review_preset_object))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/context-archive", get(super::context_archives::export))
        .route("/api/projects/{project_id}/context-imports/preview", post(super::context_archives::preview))
        .route("/api/projects/{project_id}/context-imports", post(super::context_archives::confirm))
        .route("/api/projects/{project_id}/context-imports/{command_id}", get(super::context_archives::receipt))
        .route("/api/projects/{project_id}/archived-contexts", get(super::context_archives::list))
        .route("/api/projects/{project_id}/archived-contexts/{context_id}", get(super::context_archives::get))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/task-navigation", get(super::agent_ui::tasks))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/workspace", get(super::agent_ui::snapshot))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/capability-readiness", get(super::mainline_capability::get))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/model-usage", get(super::task_model_usage::list))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/model-usage/attempts/{attempt_id}", get(super::task_model_usage::get))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/advance", post(super::agent_ui::advance))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/thread", get(super::agent_ui::thread))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/send", post(super::conversations::send))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/agent-model", get(super::conversations::agent_model).post(super::conversations::select_agent_model))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/send/{message_id}", get(super::conversations::send_receipt))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/message-queue", get(super::conversations::message_queue))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/message-queue/{message_id}/cancel", post(super::conversations::cancel_queued_message))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/message-queue/{message_id}/schema-preview", get(super::conversation_queue::preview))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/message-queue/{message_id}/schema-authorization", get(super::conversation_queue::authorization))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/message-queue/{message_id}/schema-proposals", post(super::conversation_queue::propose))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/stop-requests", post(super::conversation_stop::begin))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/stop-requests/{message_id}", get(super::conversation_stop::get))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/stop-requests/{message_id}/select", post(super::conversation_stop::select))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/task-selection", get(super::conversations::selection).post(super::conversations::select_task))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/budget", get(super::processing_operations::conversation_budget))
        .route("/api/projects/{project_id}/conversation-call-limit", get(super::conversations::call_limit).post(super::conversations::set_call_limit))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/processing-operations", get(super::processing_operations::conversation_history))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/human-requests", get(super::conversation_human_requests::list).post(super::conversation_human_requests::create))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/human-requests/{request_id}/answer", post(super::conversation_human_requests::answer))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/human-requests/{request_id}/cancel", post(super::conversation_human_requests::cancel))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/human-requests/{request_id}/deferral", post(super::conversation_human_requests::defer))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/human-requests/{request_id}/resume", post(super::conversation_human_requests::resume))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/sample-preview", get(super::sample_operations::conversation_preview))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/sample-operations", get(super::sample_operations::conversation_history))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/visual-selections", get(super::sample_operations::visual_selections))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/builder-preview", get(super::conversation_builder::preview))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/journey-preview", get(super::conversation_journey::preview))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/journey-consents", get(super::conversation_journey::history).post(super::conversation_journey::save))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/journey-consents/{consent_id}", get(super::conversation_journey::get))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/journey-consents/{consent_id}/execution", get(super::conversation_journey::status).post(super::conversation_journey::execute))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/journey-consents/{consent_id}/revoke", post(super::conversation_journey::revoke))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/builder-operations", get(super::conversation_builder::history).post(super::conversation_builder::launch))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/cancellations", get(super::conversation_schema::cancellations))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/calls", get(super::conversation_schema::history))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/calls/{call_id}/cancel", post(super::conversation_schema::cancel))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/schema-preview", get(super::conversation_schema::preview))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback-preview", get(super::conversation_feedback::preview))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback", get(super::conversation_feedback::for_message))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback-authorizations", post(super::conversation_feedback::authorize))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}", get(super::conversation_feedback::status))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}/execute", post(super::conversation_feedback::execute))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}/human-request", post(super::conversation_feedback::human_request))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}/scope-answer", post(super::conversation_feedback::answer_scope))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}/image-class-preview", get(super::conversation_image_class::preview))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}/image-class", get(super::conversation_image_class::for_feedback).post(super::conversation_image_class::create))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/image-class-reviews/{review_id}", get(super::conversation_image_class::get))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/image-class-reviews/{review_id}/answer", post(super::conversation_image_class::answer))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/image-class-reviews/{review_id}/resume", post(super::conversation_image_class::resume))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/image-class-reviews/{review_id}/cancel", post(super::conversation_image_class::cancel))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}/future-schema", get(super::conversation_feedback::future_schema).post(super::conversation_feedback::save_future_schema))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}/future-schema/proposal-preview", get(super::conversation_future_proposal::preview))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}/future-schema/proposal", get(super::conversation_future_proposal::get).post(super::conversation_future_proposal::authorize))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/feedback/{call_id}/future-schema/proposal/execute", post(super::conversation_future_proposal::execute))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/calls/{call_id}/clarification", get(super::conversation_schema::clarification))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/calls/{call_id}/clarification/answer", post(super::conversation_schema::answer_clarification))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/calls/{call_id}/clarification/cancel", post(super::conversation_schema::cancel_clarification))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/schema-authorizations/pending", get(super::conversation_schema::pending_authorization))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/schema-proposals", post(super::conversation_schema::propose))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/human-schema-drafts", get(super::conversation_schema::human_drafts).post(super::conversation_schema::save_human_draft))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/calls/{call_id}/schema-draft", get(super::conversation_schema::draft_for_call).post(super::conversation_schema::save_draft))
        .route("/api/projects/{project_id}/conversation-schema-drafts/{draft_id}", get(super::conversation_schema::read_draft).post(super::conversation_schema::edit_draft))
        .route("/api/projects/{project_id}/conversations/{conversation_id}/tasks/{task_id}/calls/{call_id}", get(super::conversation_schema::receipt))
        .route(
            "/api/projects/{project_id}/conversations",
            get(super::conversations::current).post(super::conversations::create),
        )
        .route(
            "/api/projects/{project_id}/conversations/{conversation_id}/messages",
            get(super::conversations::messages).post(super::conversations::append),
        )
        .route(
            "/api/projects/{project_id}/conversations/{conversation_id}/messages/{message_id}",
            get(super::conversations::message),
        )
        .route(
            "/api/projects/{project_id}/conversations/{conversation_id}/tasks",
            get(super::conversations::tasks).post(super::conversations::begin_task),
        )
        .route(
            "/api/projects/{project_id}/processing-preview",
            get(super::processing_operations::preview),
        )
        .route(
            "/api/projects/{project_id}/processing-operations",
            post(super::processing_operations::confirm),
        )
        .route(
            "/api/projects/{project_id}/processing-operations/{operation_id}",
            get(super::processing_operations::get),
        )
        .route(
            "/api/projects/{project_id}/sample-operations",
            post(super::sample_operations::start_operation),
        )
        .route(
            "/api/projects/{project_id}/sample-operations/{operation_id}",
            get(super::sample_operations::get_operation),
        )
        .route(
            "/api/projects/{project_id}/sample-operations/{operation_id}/cancel",
            post(super::sample_operations::cancel_operation),
        )
        .route(
            "/api/projects/{project_id}/goal",
            get(super::get_project_goal).put(super::put_project_goal),
        )
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
            "/api/projects/{project_id}/image-upload",
            post(upload_project_image)
                .layer(axum::extract::DefaultBodyLimit::max(25 * 1024 * 1024)),
        )
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
        .route(
            "/api/projects/{project_id}/images/{index}/thumbnail",
            get(super::image_previews::thumbnail),
        )
        .route("/api/projects/{project_id}/runs", post(start_run))
        .route(
            "/api/projects/{project_id}/management/preview",
            post(preview_project_management),
        )
        .route(
            "/api/projects/{project_id}/management/actions",
            post(execute_project_management),
        )
        .route(
            "/api/projects/{project_id}/management/operations/{operation_id}",
            get(get_project_management_operation),
        )
        .route(
            "/api/projects/{project_id}/management/usage",
            get(get_project_management_usage),
        )
        .route("/api/projects/{project_id}/trash", get(list_project_trash))
        .route(
            "/api/projects/{project_id}/pipelines",
            get(list_project_pipeline_lifecycle),
        )
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
        .route("/api/projects/{project}/conversations/{conversation}/tasks/{task}/exports/events",get(super::export_jobs::events))
        .route("/api/projects/{project}/conversations/{conversation}/tasks/{task}/exports/{id}",get(super::export_jobs::status))
        .route("/api/projects/{project_id}/conversations/{conversation}/tasks/{task}/exports", get(super::conversation_export_history))
        .route("/api/projects/{project_id}/exports/{export_id}/download", get(super::download_export))
        .route("/api/runs/{run_id}", get(get_run))
        .route("/api/runs/{run_id}/provenance", get(get_run_provenance))
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
            post(replay_run_from_node).get(super::replay_commands::preview),
        )
        .route(
            "/api/runs/{run_id}/replay/{node_id}/commands/{command_id}",
            get(super::replay_commands::receipt),
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
