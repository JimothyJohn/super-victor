//! Integration tests for gen_certs.sh with REAL openssl — no mocks. The
//! mocked command tests previously passed while the script itself didn't
//! exist; this file is the regression against that class of drift.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../endpoint/scripts/gen_certs.sh")
        .canonicalize()
        .expect("gen_certs.sh must exist — `qs certs` invokes it")
}

fn tmp_workdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gen-certs-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_script(cwd: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new("bash");
    cmd.arg(script_path()).args(args).current_dir(cwd);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().expect("spawn bash")
}

fn openssl(cwd: &Path, args: &[&str]) -> Output {
    Command::new("openssl")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("spawn openssl")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn full_issuance_chain_verifies_against_ca() {
    let dir = tmp_workdir("chain");

    for args in [
        vec!["ca"],
        vec!["device", "esp32"],
        vec!["server", "caddy", "127.0.0.1"],
    ] {
        let out = run_script(&dir, &args, &[]);
        assert!(out.status.success(), "{args:?} failed: {out:?}");
    }
    let out = run_script(&dir, &["admin", "opsuser"], &[("P12_PASSWORD", "test-pw")]);
    assert!(out.status.success(), "admin failed: {out:?}");

    // Every issued cert must chain to the CA.
    let verify = openssl(
        &dir,
        &[
            "verify",
            "-CAfile",
            "certs/ca/ca.pem",
            "certs/devices/esp32/client.pem",
            "certs/servers/caddy/server.pem",
            "certs/admins/opsuser/admin.pem",
        ],
    );
    let verdict = stdout(&verify);
    assert!(verify.status.success(), "chain verify failed: {verdict}");
    assert_eq!(verdict.matches(": OK").count(), 3, "verdict: {verdict}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn admin_cert_subject_satisfies_dashboard_gate() {
    let dir = tmp_workdir("admin");
    run_script(&dir, &["ca"], &[]);
    let out = run_script(&dir, &["admin", "nick"], &[("P12_PASSWORD", "pw")]);
    assert!(out.status.success(), "admin failed: {out:?}");

    // RFC2253 gives stable "K=V,K=V" output on both OpenSSL 3 (Linux prints
    // "OU = admin" by default) and LibreSSL (macOS prints "OU=admin").
    let subject = stdout(&openssl(
        &dir,
        &[
            "x509",
            "-in",
            "certs/admins/nick/admin.pem",
            "-noout",
            "-subject",
            "-nameopt",
            "RFC2253",
        ],
    ));
    // The exact component the endpoint's is_admin_subject() matches on
    // (its parser also trims spaces around '=', so both raw formats pass
    // the gate itself).
    assert!(
        subject.contains("OU=admin"),
        "admin cert must carry OU=admin: {subject}"
    );
    assert!(subject.contains("CN=nick"), "subject: {subject}");

    // P12 bundle must open with the supplied password and contain the chain.
    let p12 = openssl(
        &dir,
        &[
            "pkcs12",
            "-info",
            "-in",
            "certs/admins/nick/admin.p12",
            "-passin",
            "pass:pw",
            "-nokeys",
        ],
    );
    assert!(p12.status.success(), "p12 must open with password");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn refuses_overwrite_without_force() {
    let dir = tmp_workdir("overwrite");
    run_script(&dir, &["ca"], &[]);

    let again = run_script(&dir, &["ca"], &[]);
    assert!(
        !again.status.success(),
        "second `ca` must refuse to clobber"
    );
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("already exists"),
        "should explain the refusal"
    );

    let forced = run_script(&dir, &["ca"], &[("FORCE", "1")]);
    assert!(forced.status.success(), "FORCE=1 must allow regeneration");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn signing_without_ca_fails_with_guidance() {
    let dir = tmp_workdir("noca");
    let out = run_script(&dir, &["device", "esp32"], &[]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("qs certs ca"),
        "error should tell the user how to create the CA"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn private_keys_are_owner_only() {
    let dir = tmp_workdir("perms");
    run_script(&dir, &["ca"], &[]);
    run_script(&dir, &["admin", "sec"], &[("P12_PASSWORD", "pw")]);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for key in ["certs/ca/ca.key", "certs/admins/sec/admin.key"] {
            let mode = std::fs::metadata(dir.join(key))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{key} must be 0600, got {mode:o}");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}
