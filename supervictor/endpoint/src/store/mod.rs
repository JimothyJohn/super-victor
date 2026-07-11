/// Store backend factory for runtime selection.
pub mod factory;
/// SQLite-backed device store (feature-gated).
#[cfg(feature = "sqlite")]
pub mod sqlite;

/// DynamoDB-backed device store (feature-gated).
#[cfg(feature = "dynamo")]
pub mod dynamo;

use crate::error::AppError;
use crate::models::{DeviceRecord, UplinkRecord};

/// Typed failure channel for storage backends. Not-found and conflict are
/// already first-class [`AppError`] variants; this classifies everything
/// else so callers can branch (and log/alert) without string matching.
#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    /// Backend engine or I/O failure (SQLite error, DynamoDB service error).
    Io {
        /// Operation that failed, e.g. `"get_device query"`.
        op: &'static str,
        /// Backend-reported detail.
        detail: String,
    },
    /// Payload (de)serialization failure.
    Serde {
        /// Operation that failed.
        op: &'static str,
        /// Serializer-reported detail.
        detail: String,
    },
    /// A shared lock was poisoned by a panicking writer.
    Poisoned,
}

impl StoreError {
    /// Backend I/O failure during `op`.
    pub fn io(op: &'static str, detail: impl core::fmt::Display) -> Self {
        StoreError::Io {
            op,
            detail: detail.to_string(),
        }
    }

    /// Serialization failure during `op`.
    pub fn serde(op: &'static str, detail: impl core::fmt::Display) -> Self {
        StoreError::Serde {
            op,
            detail: detail.to_string(),
        }
    }
}

impl core::fmt::Display for StoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StoreError::Io { op, detail } => write!(f, "{op}: {detail}"),
            StoreError::Serde { op, detail } => write!(f, "{op} (serde): {detail}"),
            StoreError::Poisoned => f.write_str("store lock poisoned"),
        }
    }
}

/// Trait abstracting device and uplink persistence.
///
/// Implementations must be `Send + Sync` for use as shared axum state.
pub trait DeviceStore: Send + Sync {
    /// Insert a new device record. Returns an error if the device ID already exists.
    fn put_device(&self, record: DeviceRecord) -> Result<DeviceRecord, AppError>;
    /// Retrieve a device by its identifier, or `None` if not found.
    fn get_device(&self, device_id: &str) -> Result<Option<DeviceRecord>, AppError>;
    /// List all registered devices.
    fn list_devices(&self) -> Result<Vec<DeviceRecord>, AppError>;
    /// Persist an uplink message.
    fn put_uplink(&self, record: UplinkRecord) -> Result<(), AppError>;
    /// Retrieve the most recent uplinks for a device, up to `limit`.
    fn get_uplinks(&self, device_id: &str, limit: usize) -> Result<Vec<UplinkRecord>, AppError>;
    /// Update a device's lifecycle status, returning the updated record.
    /// Errors with [`AppError::DeviceNotFound`] if the device does not exist.
    fn set_device_status(&self, device_id: &str, status: &str) -> Result<DeviceRecord, AppError>;
    /// Most recent uplink per device (devices that never uplinked are
    /// absent). One query on SQLite; per-device on DynamoDB (see impl notes).
    fn latest_uplinks(&self) -> Result<Vec<UplinkRecord>, AppError>;
}
