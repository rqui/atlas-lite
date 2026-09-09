# Atlas Lite implementation index

Implementation requirements live in this repository. Chat prompts select the task, working branch and stopping point; they do not replace the product plan.

## Read order

1. [`../../AGENTS.md`](../../AGENTS.md) — direct development, Git, security and evidence rules.
2. [`ATLAS-LITE-01.md`](ATLAS-LITE-01.md) — initial product roadmap, retaining the established milestone numbers.
3. [`ATLAS-LITE-VOICE-FIRST-DECISION.md`](ATLAS-LITE-VOICE-FIRST-DECISION.md) — dated product decision: skip M6; Capture means voice.
4. [`ATLAS-LITE-RC-FIXES-01.md`](ATLAS-LITE-RC-FIXES-01.md) — current, narrowly scoped installation and integration corrections after M8.
5. [`../ATLAS_LITE_ARCHITECTURE.md`](../ATLAS_LITE_ARCHITECTURE.md) and [`../UPSTREAM.md`](../UPSTREAM.md) — architecture and fork relationship.
6. [`ATLAS-LITE-DESIGN.md`](ATLAS-LITE-DESIGN.md) — preserved original design baseline, not an instruction to reimplement completed work. Its old manual text-capture requirements are superseded by the dated voice-first decision.

## Product roadmap

| Milestone | Product intent |
| --- | --- |
| M0 | Fork, provenance and initial planning |
| M1 | Hardware-preserving Atlas shell |
| M1.5 | Native simulator reusing product state and rendering |
| M2 | Configuration/NVS, typed AtlasClient and HTTPS |
| M3 | Home, Library, Note and bounded Markdown reader |
| M4 | Search and Views using the same reader |
| M5 | Bounded cache and durable idempotent pending operations |
| M6 | CANCELLED: no standalone manual Text Capture UI |
| M7 | Voice recording, durable upload, original audio and automatic server transcription |
| M8 | Product setup/pairing, Settings, update/recovery readiness and power policy |

These are scope identifiers, not physical completion claims. Published PRs and their exact-head evidence describe implementation status. Physical validation remains a separate gate.

## Current execution

Execute only [`ATLAS-LITE-RC-FIXES-01.md`](ATLAS-LITE-RC-FIXES-01.md). This is a corrective close-out, not a new functional milestone and not a restart of M0–M8.

The retired microtask/orchestration plans have been removed from the active tree. Their history remains in Git. No external workflow installation or agent hierarchy is required.

Relevant implementation documentation:

- [`../M8_PRODUCTIZATION.md`](../M8_PRODUCTIZATION.md)
- [`../POWER_INPUT.md`](../POWER_INPUT.md)
- [`../VOICE_CAPTURE.md`](../VOICE_CAPTURE.md)
- [`../SIMULATION.md`](../SIMULATION.md)
- [`../RELEASE.md`](../RELEASE.md)
- [`../PHYSICAL_SMOKE_TEST.md`](../PHYSICAL_SMOKE_TEST.md)
- [`../KNOWN_ISSUES.md`](../KNOWN_ISSUES.md)

Keep these documents accurate as corrections land. Internal scratch notes and build artifacts are not product documentation.
