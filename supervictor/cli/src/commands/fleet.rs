use std::path::PathBuf;

use serde::Deserialize;

use crate::commands::ping::build_mtls_agent;
use crate::config::ProjectConfig;
use crate::error::CliError;
use crate::runner;

/// Arguments for the `qs fleet` command.
pub struct FleetArgs {
    /// Admin identity whose cert to present (certs/admins/<name>/).
    pub admin: String,
    /// Optional CA cert path (default: certs/ca/ca.pem if present, else WebPki).
    pub ca: Option<PathBuf>,
    /// Target hostname.
    pub host: String,
    /// Target HTTPS port.
    pub port: u16,
    /// Print commands without executing.
    pub dry_run: bool,
}

/// One device row from `GET /fleet` (mirrors the endpoint's `FleetDevice`).
#[derive(Debug, Deserialize)]
pub struct FleetDevice {
    /// Unique device identifier.
    pub device_id: String,
    /// Owner of the device.
    pub owner_id: String,
    /// Lifecycle status.
    pub status: String,
    /// Most recent uplink timestamp, if any.
    #[serde(default)]
    pub last_uplink: Option<String>,
    /// Freshness classification.
    pub staleness: String,
    /// Firmware version last reported, if any.
    #[serde(default)]
    pub fw: Option<String>,
}

/// Fetch fleet health over admin mTLS and print it as a table.
pub fn run_fleet(args: &FleetArgs, config: &ProjectConfig) -> Result<i32, CliError> {
    let admin_dir = config
        .repo_root
        .join(format!("certs/admins/{}", args.admin));
    let cert = admin_dir.join("admin.pem");
    let key = admin_dir.join("admin.key");
    for (path, label) in [(&cert, "admin cert"), (&key, "admin key")] {
        if !path.exists() {
            runner::error(&format!(
                "{} not found at {} (issue one with: qs certs admin {})",
                label,
                path.display(),
                args.admin
            ));
            return Ok(1);
        }
    }

    // Default to the project CA when it exists (local Caddy / staging);
    // WebPki roots otherwise (API Gateway's public cert).
    let default_ca = config.repo_root.join("certs/ca/ca.pem");
    let ca = args
        .ca
        .clone()
        .or_else(|| default_ca.exists().then_some(default_ca));

    let url = format!("https://{}:{}/fleet", args.host, args.port);
    runner::step(&format!("Fetching {}", url));
    if args.dry_run {
        println!("  [dry-run] GET {}", url);
        return Ok(0);
    }

    let agent = build_mtls_agent(ca.as_deref(), &cert, &key)?;
    let devices: Vec<FleetDevice> = match agent.get(&url).call() {
        Ok(response) => {
            let body = response.into_body().read_to_string().unwrap_or_default();
            serde_json::from_str(&body)
                .map_err(|e| CliError::Config(format!("unexpected /fleet response: {e}")))?
        }
        Err(ureq::Error::StatusCode(status)) => {
            runner::error(&format!("endpoint returned HTTP {status}"));
            return Ok(1);
        }
        Err(e) => {
            runner::error(&format!("request failed: {e}"));
            return Ok(1);
        }
    };

    print!("{}", render_table(&devices));
    Ok(0)
}

/// Render fleet rows as an aligned terminal table with a summary line.
pub fn render_table(devices: &[FleetDevice]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<24} {:<10} {:<12} {:<9} {:<12} {}\n",
        "DEVICE", "STATUS", "OWNER", "FRESH", "FW", "LAST UPLINK"
    ));
    for d in devices {
        out.push_str(&format!(
            "{:<24} {:<10} {:<12} {:<9} {:<12} {}\n",
            d.device_id,
            d.status,
            d.owner_id,
            d.staleness,
            d.fw.as_deref().unwrap_or("-"),
            d.last_uplink.as_deref().unwrap_or("never"),
        ));
    }
    let active = devices.iter().filter(|d| d.status == "active").count();
    let dark = devices
        .iter()
        .filter(|d| d.status == "active" && d.staleness == "dark")
        .count();
    out.push_str(&format!(
        "{} device(s), {} active, {} dark\n",
        devices.len(),
        active,
        dark
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, status: &str, staleness: &str, fw: Option<&str>) -> FleetDevice {
        FleetDevice {
            device_id: id.into(),
            owner_id: "owner".into(),
            status: status.into(),
            last_uplink: (staleness != "unknown").then(|| "2026-07-11T00:00:00+00:00".into()),
            staleness: staleness.into(),
            fw: fw.map(Into::into),
        }
    }

    #[test]
    fn table_renders_all_rows_and_summary() {
        let devices = vec![
            dev("factory-01", "active", "fresh", Some("0.1.0")),
            dev("factory-02", "active", "dark", None),
            dev("retired", "inactive", "dark", Some("0.0.9")),
        ];
        let table = render_table(&devices);
        assert!(table.contains("factory-01"));
        assert!(table.contains("0.1.0"));
        assert!(table.contains("never") || table.contains("2026-07-11"));
        // Summary counts only active devices as dark.
        assert!(table.contains("3 device(s), 2 active, 1 dark"));
    }

    #[test]
    fn table_parses_endpoint_json_shape() {
        // Exact shape the endpoint serializes (fw/last_uplink omitted when None).
        let json = r#"[
            {"device_id":"a","owner_id":"o","status":"active",
             "last_uplink":"2026-07-11T00:00:00+00:00","staleness":"fresh","fw":"1.2.3"},
            {"device_id":"b","owner_id":"o","status":"active","staleness":"unknown"}
        ]"#;
        let devices: Vec<FleetDevice> = serde_json::from_str(json).unwrap();
        let table = render_table(&devices);
        assert!(table.contains("1.2.3"));
        assert!(table.contains("never"));
    }
}
