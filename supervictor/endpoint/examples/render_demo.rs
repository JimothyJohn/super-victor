//! Render a static, seeded copy of the fleet dashboard for the GitHub Pages
//! demo. The templates are pure functions, so this is just: canned data →
//! Markup → HTML files, with the live-SSE script swapped for a client-side
//! simulator and absolute `/ui/...` paths rewritten to relative ones.
//!
//! Usage: cargo run -p supervictor-endpoint --example render_demo -- <outdir>

use std::fs;
use std::path::Path;

use supervictor_endpoint::models::{DeviceRecord, UplinkRecord};
use supervictor_endpoint::ui::views::{self, FleetRow, Staleness};
use supervictor_endpoint::ui::{assets, views::LIVE_SCRIPT};

const DEMO_SUBJECT: &str = "CN=demo-viewer,OU=admin,O=supervictor";
const DEMO_CSRF: &str = "static-demo";

/// Client-side simulator: fakes a live feed and disables mutating forms.
const DEMO_SCRIPT: &str = r#"
(function () {
  var dot = document.getElementById('live-dot');
  if (dot) { dot.textContent = 'demo'; dot.className = 'connected'; }
  document.querySelectorAll('form').forEach(function (form) {
    form.addEventListener('submit', function (e) {
      e.preventDefault();
      var button = form.querySelector('button');
      if (!button) return;
      var original = button.textContent;
      button.textContent = 'demo only';
      setTimeout(function () { button.textContent = original; }, 1200);
    });
  });
  var rows = Array.prototype.slice.call(document.querySelectorAll('tr[data-device]'));
  if (!rows.length) return;
  setInterval(function () {
    var row = rows[Math.floor(Math.random() * rows.length)];
    var last = row.querySelector('.last-uplink');
    if (last) last.textContent = new Date().toISOString().replace(/\.\d+Z$/, '+00:00');
    var badge = row.querySelector('.staleness');
    if (badge) { badge.textContent = 'fresh'; badge.className = 'badge staleness fresh'; }
  }, 2500);
})();
"#;

fn device(id: &str, status: &str, subject: Option<&str>, created: &str) -> DeviceRecord {
    DeviceRecord {
        device_id: id.into(),
        owner_id: "advin".into(),
        subject_dn: subject.map(Into::into),
        status: status.into(),
        created_at: created.into(),
    }
}

fn uplink(id: &str, at: &str, current: i32) -> UplinkRecord {
    UplinkRecord {
        device_id: id.into(),
        received_at: at.into(),
        payload: serde_json::json!({ "current": current }),
    }
}

/// Rewrite app-absolute paths and the SSE script for static hosting.
fn staticize(html: String, devices: &[&str]) -> String {
    let mut out = html
        .replace("href=\"/ui/assets/style.css\"", "href=\"style.css\"")
        .replace("href=\"/ui\"", "href=\"index.html\"")
        .replace("action=\"/ui/devices\"", "action=\"#\"")
        .replace(LIVE_SCRIPT, DEMO_SCRIPT);
    for id in devices {
        out = out
            .replace(
                &format!("href=\"/ui/devices/{id}\""),
                &format!("href=\"{id}.html\""),
            )
            .replace(
                &format!("action=\"/ui/devices/{id}/status\""),
                "action=\"#\"",
            );
    }
    out
}

fn main() {
    let outdir = std::env::args().nth(1).unwrap_or_else(|| "demo-out".into());
    let outdir = Path::new(&outdir);
    fs::create_dir_all(outdir).expect("create output dir");

    let devices = [
        device(
            "factory-01",
            "active",
            Some("CN=factory-01,OU=devices,O=supervictor"),
            "2026-05-02T14:11:09+00:00",
        ),
        device(
            "factory-02",
            "active",
            Some("CN=factory-02,OU=devices,O=supervictor"),
            "2026-05-02T14:26:44+00:00",
        ),
        device(
            "warehouse-07",
            "inactive",
            Some("CN=warehouse-07,OU=devices,O=supervictor"),
            "2026-03-19T09:02:31+00:00",
        ),
        device("lab-bench-3", "active", None, "2026-07-09T21:47:00+00:00"),
    ];
    let ids: Vec<&str> = devices.iter().map(|d| d.device_id.as_str()).collect();

    let rows = vec![
        FleetRow {
            device: devices[0].clone(),
            last_uplink: Some("2026-07-11T03:58:12+00:00".into()),
            staleness: Staleness::Fresh,
        },
        FleetRow {
            device: devices[1].clone(),
            last_uplink: Some("2026-07-11T02:31:55+00:00".into()),
            staleness: Staleness::Stale,
        },
        FleetRow {
            device: devices[2].clone(),
            last_uplink: Some("2026-06-28T17:04:40+00:00".into()),
            staleness: Staleness::Dark,
        },
        FleetRow {
            device: devices[3].clone(),
            last_uplink: None,
            staleness: Staleness::Unknown,
        },
    ];

    let fleet = views::fleet_page(&rows, DEMO_CSRF, DEMO_SUBJECT).into_string();
    fs::write(outdir.join("index.html"), staticize(fleet, &ids)).expect("write fleet page");

    // Detail pages: a lively one for factory-01, sparser ones for the rest.
    let history = [
        ("2026-07-11T03:58:12+00:00", 41),
        ("2026-07-11T03:53:11+00:00", 44),
        ("2026-07-11T03:48:14+00:00", 39),
        ("2026-07-11T03:43:09+00:00", 47),
        ("2026-07-11T03:38:13+00:00", 52),
        ("2026-07-11T03:33:10+00:00", 45),
        ("2026-07-11T03:28:12+00:00", 38),
        ("2026-07-11T03:23:08+00:00", 42),
    ];
    for record in &devices {
        let uplinks: Vec<UplinkRecord> = if record.device_id == "factory-01" {
            history
                .iter()
                .map(|(at, current)| uplink("factory-01", at, *current))
                .collect()
        } else if record.device_id == "factory-02" {
            vec![uplink("factory-02", "2026-07-11T02:31:55+00:00", 12)]
        } else {
            Vec::new()
        };
        let page = views::device_page(record, &uplinks, DEMO_CSRF, DEMO_SUBJECT).into_string();
        fs::write(
            outdir.join(format!("{}.html", record.device_id)),
            staticize(page, &ids),
        )
        .expect("write device page");
    }

    fs::write(outdir.join("style.css"), assets::STYLESHEET).expect("write stylesheet");
    println!(
        "demo rendered: {} pages -> {}",
        devices.len() + 1,
        outdir.display()
    );
}
