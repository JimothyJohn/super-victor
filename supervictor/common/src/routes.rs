/// Root endpoint — hello GET + uplink POST
pub const ROOT: &str = "/";

/// Health check
pub const HEALTH: &str = "/health";

/// Device collection — GET (list), POST (register)
pub const DEVICES: &str = "/devices";

/// Single device — GET. Axum pattern: /devices/{device_id}
pub const DEVICE_PATTERN: &str = "/devices/{device_id}";

/// Device uplinks — GET. Axum pattern: /devices/{device_id}/uplinks
pub const DEVICE_UPLINKS_PATTERN: &str = "/devices/{device_id}/uplinks";

/// Fleet status — GET (admin mTLS). Machine-readable per-device health.
pub const FLEET: &str = "/fleet";

/// Fleet summary — GET (admin mTLS). Aggregate counts for alerting.
pub const FLEET_SUMMARY: &str = "/fleet/summary";
