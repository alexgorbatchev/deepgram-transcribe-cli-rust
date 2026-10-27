---
created_on: 2026-10-14 18:58
last_modified: 2026-10-14 22:40
status: current
---

# Deepgram transcription library

This internal workspace package contains reusable functionality within `deepgram-transcribe-cli`; CLI contracts belong to the consumer. Inherit `../../AGENTS.md` and run `just check` and `just build` from this package directory. Checks place temporary files in `.tmp`, require strict Clippy and at least 90% owned-source line coverage, and compile the README example.

- Read applicable skills and original source contracts before editing. Add behavioral regression tests for runtime changes and perform a genuine disabled-change failure followed by exact restoration.
- Never use paid API calls or actual external programs in automated tests. Exercise HTTP through real local servers and audio through the native injected command-runner contract.
- Keep uploads single-attempt with redirects disabled, sensitive authentication headers, cancellation and bounded response reads. Preserve current `diarize_model`; do not send deprecated diarization switches alongside it.
- Keep pending billing distinct from a zero charge and unknown speakers distinct from zero. Preserve unmodeled API fields.
- Preserve original billed history on response-only writes and cache hits. Never invent records from raw responses or transcripts from history-only entries. Keep updates atomic and cache paths private.
- Select processed audio only after finite positive probes establish strictly shorter duration. Always remove unused temporary output; byte savings do not prove cost savings. Reject paths the string-based runner cannot represent losslessly.
- Keep progress and verification evidence outside these standing instructions. No commits, remote publication or live service writes without explicit authorization.
