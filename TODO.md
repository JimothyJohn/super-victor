# TODO — Engineering Roadmap

Prioritized backlog of concrete improvements. The scaling plan lives in
`ROADMAP.md`; larger strategic tracks in `todo/` (ENTERPRISE.md,
open-source-framework.md). Completed items are deleted in the PR that lands
them — git history is the changelog. Every remaining item is gated; the
**unblocks** note says what has to happen first.

## P0 — Security & cost guardrails

- [ ] Billing alarm: CloudWatch `EstimatedCharges` alarm + SNS topic. Shares
      the topic/endpoint with ROADMAP Phase 2's dark-device alerting.
      **Unblocks:** an alert email/endpoint decision + account-level billing
      alerts enabled.
- [ ] Cert lifecycle: `deploy-staging.sh` signs with `certs/ca/ca.key` from
      the local working tree (gitignored, single copy on one laptop). Move
      signing material to SSM Parameter Store / Secrets Manager; document
      issuance + rotation; keep only public certs local.
      **Unblocks:** an ops window — touches live staging deploys.

## P1 — Hygiene

- [ ] Reconcile `.claude/settings.json` (added to this repo mid-session by a
      concurrent session while the same files were being deleted from the
      `~/.claude` config repo — since restored there). Decide which level owns
      the config and delete the other copy.
      **Unblocks:** owner adjudication.

## P2 — Firmware (edge)

- [ ] Decide the `portal` feature's fate: it `include_bytes!`s
      `portal/dist/supervictor_portal_bg.wasm.gz`, which is not in the tree —
      the feature cannot build. Either add a portal build step (wasm
      toolchain) or drop the feature until the portal returns.
      **Unblocks:** owner decision (build it vs. drop it).
- [ ] esp ecosystem migration: bump `esp-rtos` 0.2 → 0.3 and `esp-radio`
      0.17 → 0.18, then unpin `esp-hal =1.0.0` (pinned because 1.1.x removed
      unstable APIs the older crates call). Remove the Dependabot ignore.
      **Unblocks:** flashable hardware on hand — CI only compile-checks the
      firmware; a scheduler/radio migration needs an on-device soak.
- [ ] OTA update path (prerequisite for cert rotation on deployed devices —
      see todo/ENTERPRISE.md and ROADMAP.md Phase 2).
      **Unblocks:** hardware + the esp migration above (A/B partition
      support lands with the newer esp-hal line).

## P3 — Endpoint

- [ ] Fleet dashboard Phase 4 (owner-scoped views, billing summaries).
      **Unblocks:** the enterprise data model (todo/ENTERPRISE.md).
- [ ] Dashboard on Lambda: SSE degrades to reload-fallback behind API GW
      buffering (by design); if live push matters there, that's the
      ECS-migration trigger per ROADMAP.md Phase 3.
      **Unblocks:** a real need for live push on the serverless path.
- [ ] Property tests (`proptest`) for payload validation.
      **Unblocks:** a dep review for the new dev-dependency.
- [ ] DynamoDB integration tests in CI (DynamoDB Local container or moto),
      so the `dynamo` feature is tested, not just compiled — and extend the
      store concurrency stress suite to it.
      **Unblocks:** container service in the CI job (docker-in-runner
      setup); no code blockers.

## P4 — CI/CD & release

- [ ] Periodic `cargo-mutants` run; mutation-catch rate is the real coverage
      number.
      **Unblocks:** nothing hard — needs a first baseline run to size the
      job (likely hours; schedule monthly, not per-PR).

## P5 — Enterprise track

See `todo/ENTERPRISE.md` (multi-table DynamoDB, billing sync, provisioning
workflows) and `ROADMAP.md` (when Step Functions and each orchestration layer
earn their keep — not before the enterprise features exist). Nothing here
blocks P0–P4.
