//! Fleet health: staleness classification and machine-readable snapshots.
//!
//! Shared by the JSON API (`GET /fleet`, `GET /fleet/summary`), the dashboard
//! (feature `ui`), and the staleness watchdog — one definition of "healthy".

use serde::Serialize;

use crate::error::AppError;
use crate::store::DeviceStore;
use crate::time::{now_rfc3339, parse_rfc3339_unix};
use supervictor_common::status;

/// Seconds since last uplink below which a device is "fresh".
pub const FRESH_SECS: u64 = 15 * 60;
/// Seconds since last uplink below which a device is "stale" (beyond: "dark").
pub const DARK_SECS: u64 = 2 * 60 * 60;

/// How recently a device has been heard from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Staleness {
    /// Uplinked within the fresh window.
    Fresh,
    /// Uplinked, but not recently.
    Stale,
    /// No uplink for a long time.
    Dark,
    /// Never uplinked, or timestamp unparseable.
    Unknown,
}

impl Staleness {
    /// CSS class / short label.
    pub fn label(self) -> &'static str {
        match self {
            Staleness::Fresh => "fresh",
            Staleness::Stale => "stale",
            Staleness::Dark => "dark",
            Staleness::Unknown => "unknown",
        }
    }
}

/// Classify a last-uplink timestamp against the fixed thresholds.
pub fn staleness_of(last_uplink: Option<&str>, now_unix: u64) -> Staleness {
    match last_uplink.and_then(parse_rfc3339_unix) {
        Some(at) => {
            let age = now_unix.saturating_sub(at);
            if age < FRESH_SECS {
                Staleness::Fresh
            } else if age < DARK_SECS {
                Staleness::Stale
            } else {
                Staleness::Dark
            }
        }
        None => Staleness::Unknown,
    }
}

/// One device's health, as served by `GET /fleet`.
#[derive(Debug, Serialize)]
pub struct FleetDevice {
    /// Unique device identifier.
    pub device_id: String,
    /// Owner of the device.
    pub owner_id: String,
    /// Lifecycle status (`active` / `inactive`).
    pub status: String,
    /// Most recent uplink timestamp, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_uplink: Option<String>,
    /// Freshness classification of `last_uplink`.
    pub staleness: Staleness,
    /// Firmware version last reported by the device, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fw: Option<String>,
}

/// Aggregate counts, as served by `GET /fleet/summary`.
#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct FleetSummary {
    /// Total registered devices.
    pub total: usize,
    /// Devices with `active` status.
    pub active: usize,
    /// Devices with `inactive` status.
    pub inactive: usize,
    /// Active devices heard from within the fresh window.
    pub fresh: usize,
    /// Active devices heard from, but not recently.
    pub stale: usize,
    /// Active devices silent past the dark threshold.
    pub dark: usize,
    /// Active devices that have never uplinked.
    pub never: usize,
}

/// Snapshot of every device's health at `now` (Unix seconds).
pub fn snapshot(store: &dyn DeviceStore, now_unix: u64) -> Result<Vec<FleetDevice>, AppError> {
    let devices = store.list_devices()?;
    let latest: std::collections::HashMap<String, (String, Option<String>)> = store
        .latest_uplinks()?
        .into_iter()
        .map(|u| {
            let fw = u
                .payload
                .get("fw")
                .and_then(|v| v.as_str())
                .map(String::from);
            (u.device_id, (u.received_at, fw))
        })
        .collect();

    Ok(devices
        .into_iter()
        .map(|device| {
            let (last_uplink, fw) = match latest.get(&device.device_id) {
                Some((at, fw)) => (Some(at.clone()), fw.clone()),
                None => (None, None),
            };
            let staleness = staleness_of(last_uplink.as_deref(), now_unix);
            FleetDevice {
                device_id: device.device_id,
                owner_id: device.owner_id,
                status: device.status,
                last_uplink,
                staleness,
                fw,
            }
        })
        .collect())
}

/// Aggregate a snapshot into summary counts. Staleness buckets count only
/// ACTIVE devices — a revoked device going quiet is expected, not an alert.
pub fn summarize(devices: &[FleetDevice]) -> FleetSummary {
    let mut summary = FleetSummary {
        total: devices.len(),
        ..FleetSummary::default()
    };
    for device in devices {
        if device.status == status::ACTIVE {
            summary.active += 1;
            match device.staleness {
                Staleness::Fresh => summary.fresh += 1,
                Staleness::Stale => summary.stale += 1,
                Staleness::Dark => summary.dark += 1,
                Staleness::Unknown => summary.never += 1,
            }
        } else {
            summary.inactive += 1;
        }
    }
    summary
}

/// Convenience: snapshot at the current wall-clock time.
pub fn snapshot_now(store: &dyn DeviceStore) -> Result<Vec<FleetDevice>, AppError> {
    snapshot(store, parse_rfc3339_unix(&now_rfc3339()).unwrap_or(0))
}
