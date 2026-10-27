---
created_on: 2026-10-14 19:12
last_modified: 2026-10-14 22:40
status: current
---

# Dependency replacements

Source: [Alex Gorbatchev's deepgram-transcribe-cli](https://github.com/alexgorbatchev/deepgram-transcribe-cli), MIT. Its full original license is retained. Reusable service, audio selection, cache, costs, vocabulary and Markdown functionality lives in the internal `crates/deepgram-transcription` package in this repository's Cargo workspace.

| Go dependency or primitive | Rust replacement | Purpose |
| :--- | :--- | :--- |
| Cobra/pflag | Clap 4 through `cli-command-kit`, clap_complete 4 | Native typed arguments and shell completion. |
| cobra-help-tree | `cli-command-kit` | Human/agent help, embedded skill support and display-path folding. |
| godeps | `cli-deps` | Native command runners, managed child PATH, version checks and explicit host-package operations. |
| Table formatting | comfy-table 7 | Terminal tables; agent mode uses TSV and key/value records. |
| Go context and signal notification | Tokio 1 / tokio-util 0.7 | Native SIGINT/SIGTERM and request cancellation. |
| Time | Chrono 0.4 | UTC history timestamps. |
| Concurrent cost refresh | Futures 0.3 | Ordered, bounded asynchronous billing requests. |
| SDK, JSON, hashing and audio/cache/report services | `deepgram-transcription` | Concrete Reqwest/Serde/SHA2/Tempfile APIs; see its dependency notes for the evaluated Deepgram SDK limitation. |

Native dependencies are locked in `Cargo.lock`. No Go runtime, SDK adapter or live package installation is included in verification.

Material source differences: request keys include complete structured options; cached Markdown uses original recorded options; unknown speakers remain unknown; cache status counts actual responses; history refresh preserves responses and limits concurrency; redirects and automatic retries are disabled for paid uploads. Native parser validation rejects negative job limits and unknown help targets. Cancellation exits 130. File-size metadata reports exact bytes. Embedded full-skill behavior is explicitly required by the source project and retained.
