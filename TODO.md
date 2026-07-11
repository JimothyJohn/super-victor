# TODO — Engineering Roadmap

Prioritized backlog of concrete improvements. The scaling plan lives in
`ROADMAP.md`; larger strategic tracks in `todo/` (ENTERPRISE.md,
open-source-framework.md). Completed items are deleted in the PR that lands
them — git history is the changelog.

## P0 — Security & cost guardrails

- [ ] Billing alarm: CloudWatch `EstimatedCharges` alarm + SNS topic (needs an
      alert email/endpoint decision, and account-level billing alerts enabled).
- [ ] Cert lifecycle: `deploy-staging.sh` signs with `certs/ca/ca.key` from the
      local working tree (gitignored, single copy on one laptop). Move signing
      material to SSM Parameter Store / Secrets Manager; document issuance +
      rotation; keep only public certs local.
- [ ] API Gateway request throttling / body-size limits reviewed and set
      explicitly (today: account defaults).

## P2 — Firmware (edge)

- [ ] Decide the `portal` feature's fate: it `include_bytes!`s
      `portal/dist/supervictor_portal_bg.wasm.gz`, which is not in the tree —
      the feature cannot build. Either add a portal build step (wasm toolchain)
      or drop the feature until the portal returns.
- [ ] esp ecosystem migration: bump `esp-rtos` 0.2 → 0.3 and `esp-radio`
      0.17 → 0.18, then unpin `esp-hal =1.0.0` (pinned because 1.1.x removed
      unstable APIs the older crates call). Remove the Dependabot ignore.
- [ ] `config.rs` exposes `CERT_PATH`/`CA_PATH` consts nothing reads (tls.rs
      re-derives them via `env!`). Wire them through or drop them.
- [ ] OTA update path (prerequisite for cert rotation on deployed devices —
      see todo/ENTERPRISE.md and ROADMAP.md Phase 2).

## P3 — Endpoint

- [ ] Fleet dashboard Phase 4 (owner-scoped views, billing summaries) — waits
      on the enterprise data model (todo/ENTERPRISE.md).
- [ ] Dashboard on Lambda: SSE degrades to reload-fallback behind API GW
      buffering (by design); if live push matters there, that's the
      ECS-migration trigger per ROADMAP.md Phase 3.
- [ ] Typed store errors: `AppError::Store(String)` is stringly-typed; a small
      enum (NotFound / Conflict / Io / Serde) preserves the HTTP mapping and
      lets handlers branch without string matching.
- [ ] Property tests (`proptest`) for payload validation — new dev-dep, run
      through dep review first.
- [ ] DynamoDB integration tests in CI (DynamoDB Local container or moto),
      so the `dynamo` feature is tested, not just compiled — and extend the
      store concurrency stress suite to it.

## P4 — CI/CD & release

- [ ] `cargo-audit`/`cargo-deny` job (weekly, advisories only) — complements
      Dependabot with CVE awareness between update cycles.
- [ ] Periodic `cargo-mutants` run; mutation-catch rate is the real coverage
      number.
- [ ] Post-deploy smoke as `workflow_dispatch` (Quickstart `integration`
      against `STAGING_URL`).
- [ ] Bump `actions/checkout` pin (current SHA targets Node 20, deprecated on
      runners) next Dependabot cycle or manually.

## P5 — Enterprise track

See `todo/ENTERPRISE.md` (multi-table DynamoDB, billing sync, provisioning
workflows) and `ROADMAP.md` (when Step Functions and each orchestration layer
earn their keep — not before the enterprise features exist). Nothing here
blocks P0–P4.
