//! Store-trait methods added for the fleet dashboard: status mutation and
//! last-uplink aggregation, against real SQLite.

mod common;

use supervictor_endpoint::error::AppError;
use supervictor_endpoint::models::{DeviceRecord, UplinkRecord};

fn device(id: &str) -> DeviceRecord {
    DeviceRecord {
        device_id: id.into(),
        owner_id: "owner-1".into(),
        subject_dn: None,
        status: "active".into(),
        created_at: "2026-07-01T00:00:00+00:00".into(),
    }
}

fn uplink(id: &str, at: &str) -> UplinkRecord {
    UplinkRecord {
        device_id: id.into(),
        received_at: at.into(),
        payload: serde_json::json!({ "current": 1 }),
    }
}

#[test]
fn set_device_status_updates_and_returns_record() {
    let store = common::test_store();
    store.put_device(device("dev-1")).unwrap();

    let updated = store.set_device_status("dev-1", "inactive").unwrap();
    assert_eq!(updated.status, "inactive");
    assert_eq!(updated.device_id, "dev-1");
    assert_eq!(
        store.get_device("dev-1").unwrap().unwrap().status,
        "inactive"
    );
}

#[test]
fn set_device_status_unknown_device_is_not_found() {
    let store = common::test_store();
    let err = store.set_device_status("ghost", "inactive").unwrap_err();
    assert!(
        matches!(err, AppError::DeviceNotFound { .. }),
        "got {err:?}"
    );
}

#[test]
fn latest_uplinks_returns_max_per_device() {
    let store = common::test_store();
    store.put_device(device("dev-1")).unwrap();
    store.put_device(device("dev-2")).unwrap();
    store.put_device(device("dev-silent")).unwrap();

    store
        .put_uplink(uplink("dev-1", "2026-07-01T10:00:00+00:00"))
        .unwrap();
    store
        .put_uplink(uplink("dev-1", "2026-07-01T12:00:00+00:00"))
        .unwrap();
    store
        .put_uplink(uplink("dev-1", "2026-07-01T11:00:00+00:00"))
        .unwrap();
    store
        .put_uplink(uplink("dev-2", "2026-06-30T00:00:00+00:00"))
        .unwrap();

    let mut latest: Vec<(String, String)> = store
        .latest_uplinks()
        .unwrap()
        .into_iter()
        .map(|u| (u.device_id, u.received_at))
        .collect();
    latest.sort();
    assert_eq!(
        latest,
        vec![
            ("dev-1".to_string(), "2026-07-01T12:00:00+00:00".to_string()),
            ("dev-2".to_string(), "2026-06-30T00:00:00+00:00".to_string()),
        ],
        "max per device, silent devices absent"
    );
}

#[test]
fn latest_uplinks_carries_the_max_rows_payload() {
    let store = common::test_store();
    store.put_device(device("dev-1")).unwrap();
    store
        .put_uplink(UplinkRecord {
            device_id: "dev-1".into(),
            received_at: "2026-07-01T10:00:00+00:00".into(),
            payload: serde_json::json!({ "current": 1, "fw": "0.0.9" }),
        })
        .unwrap();
    store
        .put_uplink(UplinkRecord {
            device_id: "dev-1".into(),
            received_at: "2026-07-01T12:00:00+00:00".into(),
            payload: serde_json::json!({ "current": 2, "fw": "0.1.0" }),
        })
        .unwrap();

    let latest = store.latest_uplinks().unwrap();
    assert_eq!(latest.len(), 1);
    assert_eq!(
        latest[0].payload["fw"], "0.1.0",
        "payload must come from the newest row, not an arbitrary one"
    );
}

#[test]
fn latest_uplinks_empty_store() {
    let store = common::test_store();
    assert!(store.latest_uplinks().unwrap().is_empty());
}
