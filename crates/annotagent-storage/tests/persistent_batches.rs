use std::{
    sync::{Arc, Barrier},
    time::Duration,
};

use annotagent_core::{
    BatchBudgetLedger, BatchBudgetLimits, BatchId, BatchImageCheckpoint, BatchImageStatus,
    BatchRecord, BatchStatus, BatchUsage, ImageId, RunId,
};
use annotagent_storage::{BatchClaimResult, SqliteStore};
use chrono::{Duration as ChronoDuration, Utc};
use rust_decimal::Decimal;

fn batch(limits: BatchBudgetLimits) -> BatchRecord {
    let now = Utc::now();
    BatchRecord {
        id: BatchId::new(),
        project_id: "project".to_owned(),
        project_path: "project/project.yaml".to_owned(),
        provider: "mock".to_owned(),
        status: BatchStatus::Pending,
        max_concurrency: 4,
        workflow_version: "workflow@1".to_owned(),
        workflow_snapshot: serde_json::json!({"hash": "workflow"}),
        project_snapshot: serde_json::json!({"name": "project"}),
        budget_limits: limits,
        budget_ledger: BatchBudgetLedger::default(),
        lease_owner: None,
        lease_expires_at: None,
        event_sequence: 0,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn startup_requeues_orphaned_image_and_checkpoint_survives_reopen() {
    let directory = tempfile::tempdir().expect("temp");
    let database = directory.path().join("history.db");
    let image_id = ImageId::new();
    let batch = batch(BatchBudgetLimits::default());
    let now = Utc::now();
    {
        let store = SqliteStore::open(&database).expect("store");
        store
            .create_batch(batch.clone(), &[(image_id, "images/one.png".to_owned())])
            .expect("batch");
        store
            .acquire_batch_lease(batch.id, "worker-a", Duration::from_secs(5), now)
            .expect("first lease");
        let reservation = BatchUsage {
            request_count: 1,
            image_count: 1,
            cost: Decimal::new(25, 2),
            ..BatchUsage::default()
        };
        assert!(matches!(
            store
                .claim_batch_image(batch.id, "worker-a", &reservation, now)
                .expect("claim"),
            BatchClaimResult::Claimed(_)
        ));
    }

    let store = SqliteStore::open(&database).expect("reopened store");
    let recovered_at = now + ChronoDuration::seconds(1);
    assert_eq!(
        store
            .recover_orphaned_batch_leases(recovered_at)
            .expect("startup recovery"),
        vec![batch.id]
    );
    store
        .acquire_batch_lease(batch.id, "worker-b", Duration::from_secs(30), recovered_at)
        .expect("take over expired lease");
    let recovered = store.get_batch(batch.id).expect("batch");
    assert_eq!(recovered.budget_ledger.reserved, BatchUsage::default());
    let reservation = BatchUsage {
        request_count: 1,
        image_count: 1,
        cost: Decimal::new(25, 2),
        ..BatchUsage::default()
    };
    let claimed = store
        .claim_batch_image(batch.id, "worker-b", &reservation, recovered_at)
        .expect("reclaimed image");
    assert!(matches!(claimed, BatchClaimResult::Claimed(_)));
    let actual = BatchUsage {
        input_tokens: 10,
        output_tokens: 5,
        total_tokens: 15,
        request_count: 1,
        image_count: 1,
        cost: Decimal::new(20, 2),
    };
    let checkpoint = BatchImageCheckpoint {
        artifact_references: vec![annotagent_core::ArtifactId::new()],
        ..BatchImageCheckpoint::default()
    };
    assert!(
        store
            .finish_batch_image(
                batch.id,
                image_id,
                "worker-b",
                BatchImageStatus::Completed,
                &actual,
                &checkpoint,
                None,
                recovered_at,
            )
            .expect("finish")
    );
    assert!(
        !store
            .finish_batch_image(
                batch.id,
                image_id,
                "worker-b",
                BatchImageStatus::Completed,
                &actual,
                &checkpoint,
                None,
                recovered_at,
            )
            .expect("idempotent finish")
    );
    let finished = store
        .finalize_batch(batch.id, "worker-b", recovered_at)
        .expect("finalize");
    assert_eq!(finished.status, BatchStatus::Completed);
    assert_eq!(finished.budget_ledger.consumed, actual);
    assert_eq!(finished.budget_ledger.reserved, BatchUsage::default());
    let durable = store.batch_checkpoint(batch.id).expect("checkpoint");
    assert_eq!(durable.completed_images, vec![image_id]);
    assert_eq!(
        durable.artifact_references[&image_id],
        checkpoint.artifact_references
    );
    let events = store.list_batch_events(batch.id).expect("events");
    assert!(
        events
            .windows(2)
            .all(|pair| pair[1].sequence == pair[0].sequence + 1)
    );
    assert_eq!(
        events.last().map(|event| event.sequence),
        Some(durable.event_sequence)
    );
}

#[test]
fn concurrent_reservations_cannot_oversell_global_budget() {
    let store = Arc::new(SqliteStore::open_in_memory().expect("store"));
    let batch = batch(BatchBudgetLimits {
        max_request_count: Some(1),
        max_cost: Some(Decimal::new(50, 2)),
        ..BatchBudgetLimits::default()
    });
    let images = [
        (ImageId::new(), "images/one.png".to_owned()),
        (ImageId::new(), "images/two.png".to_owned()),
    ];
    store.create_batch(batch.clone(), &images).expect("batch");
    let now = Utc::now();
    store
        .acquire_batch_lease(batch.id, "worker", Duration::from_secs(30), now)
        .expect("lease");
    let reservation = BatchUsage {
        request_count: 1,
        image_count: 1,
        cost: Decimal::new(30, 2),
        ..BatchUsage::default()
    };
    let barrier = Arc::new(Barrier::new(3));
    let workers = (0..2)
        .map(|_| {
            let store = store.clone();
            let barrier = barrier.clone();
            let reservation = reservation.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .claim_batch_image(batch.id, "worker", &reservation, now)
                    .expect("atomic reservation")
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let claims = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker"))
        .collect::<Vec<_>>();
    assert_eq!(
        claims
            .iter()
            .filter(|claim| matches!(claim, BatchClaimResult::Claimed(_)))
            .count(),
        1
    );
    assert_eq!(
        claims
            .iter()
            .filter(|claim| matches!(claim, BatchClaimResult::BudgetExceeded(_)))
            .count(),
        1
    );
    let persisted = store.get_batch(batch.id).expect("batch");
    assert_eq!(persisted.status, BatchStatus::BudgetExceeded);
    assert_eq!(persisted.budget_ledger.reserved.request_count, 1);
    assert_eq!(persisted.budget_ledger.reserved.cost, Decimal::new(30, 2));
}

#[test]
fn cancellation_prevents_new_image_nodes_from_starting() {
    let store = SqliteStore::open_in_memory().expect("store");
    let batch = batch(BatchBudgetLimits::default());
    let images = (0..3)
        .map(|index| (ImageId::new(), format!("images/{index}.png")))
        .collect::<Vec<_>>();
    store.create_batch(batch.clone(), &images).expect("batch");
    let now = Utc::now();
    store
        .acquire_batch_lease(batch.id, "worker", Duration::from_secs(30), now)
        .expect("lease");
    let claimed = match store
        .claim_batch_image(
            batch.id,
            "worker",
            &BatchUsage {
                request_count: 1,
                image_count: 1,
                ..BatchUsage::default()
            },
            now,
        )
        .expect("claim")
    {
        BatchClaimResult::Claimed(image) => image,
        other => panic!("unexpected claim: {other:?}"),
    };
    store
        .mark_batch_image_running(batch.id, claimed.image_id, "worker", RunId::new(), now)
        .expect("started");
    store
        .set_batch_status(batch.id, BatchStatus::Cancelled, now)
        .expect("cancel");
    assert!(
        store
            .claim_batch_image(batch.id, "worker", &BatchUsage::default(), now)
            .is_err()
    );
    assert!(
        store
            .list_batch_images(batch.id)
            .expect("images")
            .iter()
            .all(|image| image.status == BatchImageStatus::Cancelled)
    );
    let events = store.list_batch_events(batch.id).expect("events");
    let cancel_sequence = events
        .iter()
        .find(|event| {
            event.kind == "batch_status_changed"
                && event.detail["status"] == serde_json::json!("cancelled")
        })
        .expect("cancel event")
        .sequence;
    assert!(
        !events
            .iter()
            .any(|event| event.sequence > cancel_sequence && event.kind == "image_started")
    );
}

#[test]
fn failed_image_retry_preserves_usage_and_does_not_repeat_completed_work() {
    let store = SqliteStore::open_in_memory().expect("store");
    let batch = batch(BatchBudgetLimits::default());
    let image_id = ImageId::new();
    let now = Utc::now();
    store
        .create_batch(batch.clone(), &[(image_id, "images/retry.png".to_owned())])
        .expect("batch");
    store
        .acquire_batch_lease(batch.id, "worker-a", Duration::from_secs(30), now)
        .expect("lease");
    let reservation = BatchUsage {
        request_count: 1,
        image_count: 1,
        ..BatchUsage::default()
    };
    assert!(matches!(
        store
            .claim_batch_image(batch.id, "worker-a", &reservation, now)
            .expect("claim"),
        BatchClaimResult::Claimed(_)
    ));
    store
        .finish_batch_image(
            batch.id,
            image_id,
            "worker-a",
            BatchImageStatus::Failed,
            &reservation,
            &BatchImageCheckpoint::default(),
            Some("transient model error"),
            now,
        )
        .expect("failed attempt");
    assert_eq!(
        store
            .finalize_batch(batch.id, "worker-a", now)
            .expect("failed batch")
            .status,
        BatchStatus::Failed
    );
    assert_eq!(
        store
            .retry_failed_batch_images(batch.id, now)
            .expect("retry failed image"),
        1
    );
    store
        .acquire_batch_lease(batch.id, "worker-b", Duration::from_secs(30), now)
        .expect("retry lease");
    assert!(matches!(
        store
            .claim_batch_image(batch.id, "worker-b", &reservation, now)
            .expect("retry claim"),
        BatchClaimResult::Claimed(_)
    ));
    store
        .finish_batch_image(
            batch.id,
            image_id,
            "worker-b",
            BatchImageStatus::Completed,
            &reservation,
            &BatchImageCheckpoint::default(),
            None,
            now,
        )
        .expect("completed retry");
    let completed = store
        .finalize_batch(batch.id, "worker-b", now)
        .expect("completed batch");
    assert_eq!(completed.status, BatchStatus::Completed);
    assert_eq!(completed.budget_ledger.consumed.request_count, 2);
    let image = store.list_batch_images(batch.id).expect("image").remove(0);
    assert_eq!(image.attempt_count, 2);
    assert_eq!(image.actual_usage.request_count, 2);
    assert!(matches!(
        store
            .claim_batch_image(batch.id, "worker-b", &reservation, now)
            .expect_err("completed batch cannot be claimed"),
        annotagent_storage::StorageError::BatchLeaseConflict(_)
    ));
}
