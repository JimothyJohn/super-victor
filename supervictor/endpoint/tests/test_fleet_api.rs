mod common;

use axum::http::{HeaderName, HeaderValue};
use axum_test::TestServer;
use supervictor_endpoint::models::{DeviceRecord, UplinkRecord};
use supervictor_endpoint::routes;
use supervictor_endpoint::store::DeviceStore;
use supervictor_endpoint::time::now_rfc3339;

const SUBJECT_HEADER: &str = "x-ssl-client-subject-dn";
const ADMIN_DN: &str = "CN=ops,OU=admin,O=supervictor";
const DEVICE_DN: &str = "CN=factory-01,OU=devices,O=supervictor";

fn server_with_store() -> (TestServer, std::sync::Arc<dyn DeviceStore>) {
    let store = common::test_store();
    let app = routes::router(store.clone());
    (TestServer::new(app), store)
}

fn dn(value: &'static str) -> (HeaderName, HeaderValue) {
    (
        HeaderName::from_static(SUBJECT_HEADER),
        HeaderValue::from_static(value),
    )
}

fn seed(store: &dyn DeviceStore, id: &str, status: &str) {
    store
        .put_device(DeviceRecord {
            device_id: id.into(),
            owner_id: "owner-1".into(),
            subject_dn: None,
            status: status.into(),
            created_at: "2026-07-01T00:00:00+00:00".into(),
        })
        .unwrap();
}

// ── Auth ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn fleet_routes_require_admin_certificate() {
    let (server, _) = server_with_store();
    for path in ["/fleet", "/fleet/summary"] {
        let resp = server.get(path).await;
        assert_eq!(resp.status_code(), 403, "GET {path} without cert");

        let (name, value) = dn(DEVICE_DN);
        let resp = server.get(path).add_header(name, value).await;
        assert_eq!(resp.status_code(), 403, "GET {path} with device cert");
    }
}

// ── /fleet ────────────────────────────────────────────────────────────

#[tokio::test]
async fn fleet_reports_staleness_and_fw() {
    let (server, store) = server_with_store();
    seed(store.as_ref(), "fresh-dev", "active");
    seed(store.as_ref(), "silent-dev", "active");
    seed(store.as_ref(), "revoked-dev", "inactive");

    // A just-now uplink carrying a firmware version.
    store
        .put_uplink(UplinkRecord {
            device_id: "fresh-dev".into(),
            received_at: now_rfc3339(),
            payload: serde_json::json!({ "current": 5, "fw": "0.1.0" }),
        })
        .unwrap();

    let (name, value) = dn(ADMIN_DN);
    let resp = server.get("/fleet").add_header(name, value).await;
    assert_eq!(resp.status_code(), 200);
    let fleet: serde_json::Value = resp.json();
    let devices = fleet.as_array().unwrap();
    assert_eq!(devices.len(), 3);

    let by_id = |id: &str| {
        devices
            .iter()
            .find(|d| d["device_id"] == id)
            .unwrap_or_else(|| panic!("{id} missing from fleet"))
    };
    assert_eq!(by_id("fresh-dev")["staleness"], "fresh");
    assert_eq!(by_id("fresh-dev")["fw"], "0.1.0");
    assert_eq!(by_id("silent-dev")["staleness"], "unknown");
    assert!(
        by_id("silent-dev").get("fw").is_none(),
        "no fw when never uplinked"
    );
    assert_eq!(by_id("revoked-dev")["status"], "inactive");
}

// ── /fleet/summary ────────────────────────────────────────────────────

#[tokio::test]
async fn summary_counts_only_active_devices_in_staleness_buckets() {
    let (server, store) = server_with_store();
    seed(store.as_ref(), "fresh-dev", "active");
    seed(store.as_ref(), "never-dev", "active");
    seed(store.as_ref(), "revoked-dev", "inactive");
    // Revoked device also uplinked long ago — must not count as dark.
    store
        .put_uplink(UplinkRecord {
            device_id: "revoked-dev".into(),
            received_at: "2026-01-01T00:00:00+00:00".into(),
            payload: serde_json::json!({ "current": 1 }),
        })
        .unwrap();
    store
        .put_uplink(UplinkRecord {
            device_id: "fresh-dev".into(),
            received_at: now_rfc3339(),
            payload: serde_json::json!({ "current": 2 }),
        })
        .unwrap();

    let (name, value) = dn(ADMIN_DN);
    let resp = server.get("/fleet/summary").add_header(name, value).await;
    assert_eq!(resp.status_code(), 200);
    let summary: serde_json::Value = resp.json();
    assert_eq!(summary["total"], 3);
    assert_eq!(summary["active"], 2);
    assert_eq!(summary["inactive"], 1);
    assert_eq!(summary["fresh"], 1);
    assert_eq!(summary["never"], 1);
    assert_eq!(summary["dark"], 0, "inactive devices don't alarm");
}

// ── End-to-end: device uplink with fw surfaces in the fleet view ─────

#[tokio::test]
async fn uplink_fw_flows_through_to_fleet() {
    let (server, store) = server_with_store();
    seed(store.as_ref(), "e2e-dev", "active");

    // Post through the real ingest route, exactly as firmware would.
    let resp = server
        .post("/")
        .text(r#"{"id":"e2e-dev","current":7,"fw":"9.9.9"}"#)
        .await;
    assert_eq!(resp.status_code(), 200);

    let (name, value) = dn(ADMIN_DN);
    let fleet: serde_json::Value = server.get("/fleet").add_header(name, value).await.json();
    assert_eq!(fleet[0]["fw"], "9.9.9");
    assert_eq!(fleet[0]["staleness"], "fresh");
}

/// Pre-fw firmware (no fw field) still ingests cleanly — rollout safety.
#[tokio::test]
async fn uplink_without_fw_still_accepted() {
    let (server, store) = server_with_store();
    seed(store.as_ref(), "old-dev", "active");
    let resp = server
        .post("/")
        .text(r#"{"id":"old-dev","current":3}"#)
        .await;
    assert_eq!(resp.status_code(), 200);

    let (name, value) = dn(ADMIN_DN);
    let fleet: serde_json::Value = server.get("/fleet").add_header(name, value).await.json();
    assert!(fleet[0].get("fw").is_none());
}
