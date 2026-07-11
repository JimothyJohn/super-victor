use heapless::String as HString;
use serde::{Deserialize, Serialize};

/// Firmware version baked in at compile time; reported with every uplink so
/// the fleet dashboard can tell which devices need updating.
pub const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Telemetry payload sent from the device to the cloud API.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UplinkMessage {
    /// Unique identifier for this device or message.
    pub id: HString<64>,
    /// Sensor reading (e.g. current in milliamps).
    pub current: i32,
    /// Firmware version this device is running (see [`FIRMWARE_VERSION`]).
    /// Defaults to empty when parsing pre-fw JSON (backward compatibility).
    #[serde(default)]
    pub fw: HString<16>,
}

impl UplinkMessage {
    /// Build an uplink stamped with this build's firmware version.
    pub fn new(id: HString<64>, current: i32) -> Self {
        Self {
            id,
            current,
            fw: FIRMWARE_VERSION.try_into().unwrap_or_default(),
        }
    }
}

/// Deserialized response from the Lambda-backed API Gateway endpoint.
#[derive(Debug, Serialize, Deserialize)]
pub struct LambdaResponse {
    /// AWS Lambda request identifier.
    #[serde(rename = "x-amzn-RequestId")]
    pub x_amzn_request_id: HString<64>,
    /// API Gateway internal request identifier.
    #[serde(rename = "x-amz-apigw-id")]
    pub x_amz_apigw_id: HString<32>,
    /// AWS X-Ray trace identifier.
    #[serde(rename = "X-Amzn-Trace-Id")]
    pub x_amzn_trace_id: HString<128>,
    /// MIME type of the response body.
    #[serde(rename = "content-type")]
    pub content_type: HString<32>,
    /// Byte length of the response body as reported by the server.
    #[serde(rename = "content-length")]
    pub content_length: HString<8>,
    /// Date header from the server response.
    pub date: HString<32>,
    /// Raw response body text.
    pub body: HString<1024>,
}
