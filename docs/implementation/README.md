# Atlas Lite implementation index

This directory is the entry point for Atlas Lite implementation work.

The repository, not chat history, is the source of implementation detail.

## Read order

1. [`../../AGENTS.md`](../../AGENTS.md) — permanent execution, Git, model/token, security and verification rules.
2. [`ATLAS-LITE-01.md`](ATLAS-LITE-01.md) — product roadmap and milestone intent.
3. [`../superpowers/plans/2026-09-04-atlas-lite-master-execution.md`](../superpowers/plans/2026-09-04-atlas-lite-master-execution.md) — detailed executable requirements for M1.5 through M8.
4. [`../superpowers/specs/2026-09-03-atlas-lite-design.md`](../superpowers/specs/2026-09-03-atlas-lite-design.md) — approved product/design baseline.
5. [`../ATLAS_LITE_ARCHITECTURE.md`](../ATLAS_LITE_ARCHITECTURE.md) — Atlas Lite architecture.
6. [`../UPSTREAM.md`](../UPSTREAM.md) — Rustmix fork/upstream strategy.

## Milestone map

| Milestone | Purpose | Detailed source |
| --- | --- | --- |
| M0 | Fork/bootstrap/planning baseline | `docs/superpowers/plans/2026-09-03-atlas-lite-m0-m1.md` |
| M1 | Hardware-preserving Atlas shell | `docs/superpowers/plans/2026-09-03-atlas-lite-m0-m1.md` |
| M1.5 | Native host simulator, mocks, QEMU spike | `docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md` |
| M2 | Secure config/NVS/provisioning/AtlasClient/HTTPS | `docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md` |
| M3 | Home, Library, Note reader, Markdown | `docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md` |
| M4 | Search and Views | `docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md` |
| M5 | Bounded offline cache and durable idempotent queue | `docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md` |
| M6 | Text Capture online/offline | `docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md` |
| M7 | Voice recording + server-side STT boundary | `docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md` |
| M8 | Power, pairing, product provisioning, OTA, release, cleanup | `docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md` |

## Prompt policy

A normal future execution prompt should **not** restate the product requirements. It should only provide temporary orchestration information, for example:

```text
Execute M2 using the repository sources of truth.
Create/verify the requested isolated branch/worktree.
Use subagent-driven development and the model/token policy in AGENTS.md.
Run all milestone validation and open a Draft PR.
Do not merge/deploy/release.
Return the milestone reporting contract from the master execution plan.
```

If a requirement is important enough to affect implementation, add it to the repository plan/spec first rather than leaving it only in a chat prompt.

## Execution artifacts

Local SDD ledgers, briefs and worker reports live under:

```text
.superpowers/sdd/
```

They are intentionally gitignored.

Durable architecture decisions, plans, specs and hardware evidence belong under `docs/`.
