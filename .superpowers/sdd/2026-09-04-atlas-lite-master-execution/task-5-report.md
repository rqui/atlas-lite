# Task 5 report — M2 narrow DTOs and bounded JSON

## Status

Implemented transport-independent, target-compatible Atlas DTO parsing. AtlasClient and HTTP
transport remain deliberately absent, and `/Users/roger/Documents/ChatGPT/Atlas` was read-only.
Implementation commit: `17400ec` (`feat: add bounded Atlas DTOs`).
Round 1 fix commit: `bf566ba` (`fix: bound Atlas DTO response collections`).

## Files

- `src/atlas_dto.rs`: narrow response DTOs for note summary pages/documents, search, View
  summaries/result pages, and canonical API errors; strict required-field parsing; private,
  pre-deserialization response-size guard and bounded collection visitors.
- `src/lib.rs`: exports the DTO module.
- `Cargo.toml` and `Cargo.lock`: direct `serde` derive and `serde_json` dependencies.
- `tests/atlas_dto.rs`: representative Atlas payload fixtures and parser/bounds coverage.

## Bounds and compatibility

- `MAX_RESPONSE_BODY_BYTES` is explicitly `64 * 1024` (65,536 bytes). The guard compares the
  received byte slice before `serde_json::from_slice`, rejecting any 65,537-byte response without
  normal JSON parsing. This accommodates compact pages and single reader documents while bounding
  the JSON input allocation/parse budget for ESP32.
- Collection limits are explicit and enforced while deserializing: `MAX_NOTE_SUMMARIES = 64`,
  `MAX_SEARCH_HITS = 64`, `MAX_VIEW_SUMMARIES = 32`, and `MAX_VIEW_RESULTS = 64`. Each visitor
  preallocates only its named maximum and rejects the first additional item before retaining it.
- DTOs retain only screen fields. Serde ignores Atlas `frontmatter`, search `score`, View
  `definition`, `columns`, `values`, relations, rollups, and future unknown fields; normal device
  flow never uses `serde_json::Value`.
- The current note-summary fixture matches the current Atlas `toNoteSummary` mapper through
  `revision`; `parentId`/`order` are covered only in a separate labelled optional-extension test.
- Required fields and canonical enum/type shapes reject malformed/missing/unexpected JSON. Current
  nullable server IDs/cursors remain required keys whose values may be `null`.

## Evidence

- FOCUSED: `cargo +stable test --target aarch64-apple-darwin --test atlas_dto` — 8/8 passed,
  including each collection-over-limit rejection.
- HOST: `./scripts/validate.sh` — 338 unit tests plus 19 integration tests passed; source and
  native-target isolation contracts passed.
- TARGET: `./scripts/build.sh` — Xtensa release build passed; ELF SHA-256
  `f146b88134a04ea31bb368c91f74bd154817d5be8fc9d7ef902e45385009127f`.
- HYGIENE: `cargo +stable fmt --all`, `git diff --check`, and staged diff check passed.

## Concerns / intentionally pending

- The 64 KiB limit is a conservative M2 input ceiling, not payload-size profiling from physical
  hardware. M2.7 transport must enforce this limit while streaming/collecting response bytes.
- This task intentionally adds no AtlasClient, HTTP handles, retries, auth headers, cache, or UI
  wiring. Existing modified `target/` artifacts remain excluded from task commits.
