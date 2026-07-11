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
fn last_uplink_times_returns_max_per_device() {
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

    let mut times = store.last_uplink_times().unwrap();
    times.sort();
    assert_eq!(
        times,
        vec![
            ("dev-1".to_string(), "2026-07-01T12:00:00+00:00".to_string()),
            ("dev-2".to_string(), "2026-06-30T00:00:00+00:00".to_string()),
        ],
        "max per device, silent devices absent"
    );
}

#[test]
fn last_uplink_times_empty_store() {
    let store = common::test_store();
    assert!(store.last_uplink_times().unwrap().is_empty());
}
