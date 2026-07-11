//! maud templates — pure functions from data to `Markup`. All interpolated
//! values are auto-escaped by maud; `PreEscaped` appears exactly once, for a
//! static script constant that contains no user data.

use maud::{html, Markup, PreEscaped, DOCTYPE};

use crate::models::{DeviceRecord, UplinkRecord};
use supervictor_common::status;

/// How recently a device has been heard from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// One row of the fleet table.
pub struct FleetRow {
    /// The device record.
    pub device: DeviceRecord,
    /// Most recent uplink timestamp, if any.
    pub last_uplink: Option<String>,
    /// Freshness classification of `last_uplink`.
    pub staleness: Staleness,
}

/// Static SSE hookup — live row updates, with a reload fallback where SSE
/// doesn't survive the proxy (Lambda/API GW buffers responses).
const LIVE_SCRIPT: &str = r#"
(function () {
  var dot = document.getElementById('live-dot');
  var errors = 0;
  var es = new EventSource('/ui/events');
  es.onopen = function () { errors = 0; if (dot) { dot.textContent = 'live'; dot.className = 'connected'; } };
  es.addEventListener('uplink', function (e) {
    var ev; try { ev = JSON.parse(e.data); } catch (_) { return; }
    var row = document.querySelector('tr[data-device="' + CSS.escape(ev.device_id) + '"]');
    if (!row) return;
    var last = row.querySelector('.last-uplink');
    if (last) last.textContent = ev.received_at;
    var badge = row.querySelector('.staleness');
    if (badge) { badge.textContent = 'fresh'; badge.className = 'badge staleness fresh'; }
  });
  es.onerror = function () {
    errors += 1;
    if (dot) { dot.textContent = 'offline'; dot.className = ''; }
    if (errors >= 3) { es.close(); setTimeout(function () { location.reload(); }, 30000); }
  };
})();
"#;

fn layout(title: &str, subject: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " — supervictor fleet" }
                link rel="stylesheet" href="/ui/assets/style.css";
            }
            body {
                header {
                    h1 { a href="/ui" { "supervictor fleet" } }
                    span #live-dot { "static" }
                    span .subject { (subject) }
                }
                main { (body) }
                script { (PreEscaped(LIVE_SCRIPT)) }
            }
        }
    }
}

/// `GET /ui` — fleet table plus the register form.
pub fn fleet_page(rows: &[FleetRow], csrf: &str, subject: &str) -> Markup {
    layout(
        "fleet",
        subject,
        html! {
            h2 { (rows.len()) " device(s)" }
            table {
                thead {
                    tr {
                        th { "device" }
                        th { "status" }
                        th { "owner" }
                        th { "last uplink" }
                        th { "freshness" }
                        th { "actions" }
                    }
                }
                tbody {
                    @for row in rows {
                        tr data-device=(row.device.device_id) {
                            td { a href={ "/ui/devices/" (row.device.device_id) } { (row.device.device_id) } }
                            td { span .badge.(row.device.status) { (row.device.status) } }
                            td { (row.device.owner_id) }
                            td .last-uplink { (row.last_uplink.as_deref().unwrap_or("never")) }
                            td { span .badge.staleness.(row.staleness.label()) { (row.staleness.label()) } }
                            td { (status_form(&row.device, csrf)) }
                        }
                    }
                }
            }
            fieldset {
                legend { "register device" }
                form method="post" action="/ui/devices" {
                    input type="hidden" name="csrf" value=(csrf);
                    input type="text" name="device_id" placeholder="device id" required;
                    input type="text" name="owner_id" placeholder="owner id" required;
                    input type="text" name="subject_dn" placeholder="subject DN (optional)";
                    button type="submit" { "register" }
                }
            }
        },
    )
}

/// `GET /ui/devices/{id}` — detail with uplink history and sparkline.
pub fn device_page(
    device: &DeviceRecord,
    uplinks: &[UplinkRecord],
    csrf: &str,
    subject: &str,
) -> Markup {
    let currents: Vec<i32> = uplinks
        .iter()
        .rev() // oldest → newest, left → right
        .filter_map(|u| u.payload.get("current").and_then(|v| v.as_i64()))
        .map(|v| v as i32)
        .collect();
    layout(
        &device.device_id,
        subject,
        html! {
            h2 { (device.device_id) }
            dl .meta {
                dt { "status" }
                dd { span .badge.(device.status) { (device.status) } " " (status_form(device, csrf)) }
                dt { "owner" }
                dd { (device.owner_id) }
                dt { "cert subject" }
                dd { (device.subject_dn.as_deref().unwrap_or("—")) }
                dt { "registered" }
                dd { (device.created_at) }
            }
            @if currents.len() >= 2 {
                h2 { "recent current readings" }
                (sparkline(&currents))
            }
            h2 { "uplinks" }
            @if uplinks.is_empty() {
                p { "no uplinks recorded" }
            } @else {
                table {
                    thead { tr { th { "received" } th { "payload" } } }
                    tbody {
                        @for uplink in uplinks {
                            tr {
                                td { (uplink.received_at) }
                                td { code { (uplink.payload.to_string()) } }
                            }
                        }
                    }
                }
            }
        },
    )
}

/// Revoke/reactivate toggle for a device (CSRF-protected form post).
fn status_form(device: &DeviceRecord, csrf: &str) -> Markup {
    let (next, verb) = if device.status == status::ACTIVE {
        (status::INACTIVE, "revoke")
    } else {
        (status::ACTIVE, "reactivate")
    };
    html! {
        form .inline method="post" action={ "/ui/devices/" (device.device_id) "/status" } {
            input type="hidden" name="csrf" value=(csrf);
            input type="hidden" name="status" value=(next);
            button type="submit" { (verb) }
        }
    }
}

/// Inline-SVG sparkline: `<polyline>`, no charting dependency.
fn sparkline(values: &[i32]) -> Markup {
    const W: f64 = 240.0;
    const H: f64 = 48.0;
    const PAD: f64 = 4.0;
    let min = *values.iter().min().unwrap_or(&0) as f64;
    let max = *values.iter().max().unwrap_or(&1) as f64;
    let span = if (max - min).abs() < f64::EPSILON {
        1.0
    } else {
        max - min
    };
    let step = (W - 2.0 * PAD) / (values.len().saturating_sub(1).max(1)) as f64;
    let points: String = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = PAD + i as f64 * step;
            let y = H - PAD - ((*v as f64 - min) / span) * (H - 2.0 * PAD);
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    html! {
        svg .sparkline width="240" height="48" viewBox={ "0 0 " (W) " " (H) } role="img" {
            polyline points=(points) {}
        }
    }
}
