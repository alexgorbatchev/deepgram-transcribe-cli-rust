---
created_on: 2026-10-14 19:12
last_modified: 2026-10-14 22:40
status: current
---

# Deepgram transcription CLI

This repository provides `deepgram-transcribe` and its internal `crates/deepgram-transcription` package in one Cargo workspace; `cli-deps` remains a sibling dependency. Consult parent instructions and the applicable CLI, documentation and language skills before implementation.

## Commands

- Run: `just run cache status`.
- Agent mode: `just run-ai cache status`.
- Test: `just test`.
- Format and lint: `just lint`.
- Coverage and strict documentation: `just check`.
- Optimized executable: `just build`, producing `bin/deepgram-transcribe`.

## Contracts

- Use native Clap parsing and shared command-tree help. Resolve runtime environment only after parsing and metadata handling; help, version, skills, completion and bare groups must work offline without cache, program discovery or credentials.
- Keep stdout strictly the requested report, Markdown or metadata. Progress and warnings belong on stderr; the entrypoint prints each returned runtime error once without usage. Preserve raw version plus newline, exit 1 on failure and 130 on cancellation.
- Preserve the source's explicitly embedded usage reference: `skills/deepgram-transcribe/SKILL.md` is the single canonical file and `skill` prints its compiled bytes verbatim. Update that file and its metadata in the same change as commands, arguments, flags, defaults, environment, output or side effects. Keep the live interface and help-alert tests complete.
- Keep all transcript and history operations in the internal library's native APIs. A reusable cache hit requires a response; preserve its original paid record and model/vocabulary metadata. `--force` bypasses reads; `--no-cache` bypasses all reads and writes. History refresh must retain a stored response and remain bounded to eight concurrent requests.
- Never retry or redirect paid audio uploads. A cancelled/failed response does not prove the service avoided charging. Keep original input contents and full effective request options in cache identity.
- Select prepared audio only when both measured durations are positive and the prepared duration is strictly shorter, including mono-only processing. Report byte changes separately. Missing/outdated tools or probe errors fall back to original audio; processing errors and cancellation fail. Retain RAII cleanup across upload, output and cache errors.
- Keep `preprocessed` as the stored request's processing intent, labeled `preprocessing_requested`; it does not prove processing ran or reduced bytes. Use the selected upload path for MIME detection.
- Derive managed child PATH without modifying process-wide environment. Invoke host package managers only for explicit dependency install/update. Keep native paths for filesystem work and shared home folding for display.
- Any runtime-result change requires a corresponding behavioral test change and at least 90% owned-source line coverage. Keep red/green and genuine source-disable/byte-exact-restore evidence under `.tmp/`; scope formatting to this package.
- Use local HTTP fixtures, isolated caches and injected program runners. Never make paid API requests, install packages, read user credentials or run actual preprocessing as an automated-test shortcut.
- Never publish releases, tags, packages or deployments automatically without explicit user authorization. Keep standing rules here and migration evidence separately.
