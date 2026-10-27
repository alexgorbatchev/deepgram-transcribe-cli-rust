`deepgram-transcription` provides audio transcription, persistent spending history and Markdown reports for Rust programs using Deepgram's prerecorded API.

# What It Does

- **Transcription:** Uploads binary audio with vocabulary, language and current speaker diarization options.
- **History:** Stores responses and billed job metadata independently, preserving history when cached audio is reused.
- **Audio selection:** Uses processed audio only when measured duration is strictly shorter.
- **Reports:** Groups adjacent speakers and distinguishes unknown speakers from speaker zero.

# How It Works

1. Choose transcription options and derive a cache key from audio contents and effective settings.
2. Read an existing response or submit audio through `Service`.
3. Save the response and job metadata, then render Markdown or retrieve actual billing.

# How it Really Works

1. Network operations are asynchronous, use a five-minute deadline per operation and accept cancellation. Uploads make one attempt; redirects and transport failures return errors without replaying billable audio. Audio and response bytes are held in memory. The caller supplies credentials explicitly.
2. Cache operations block on the filesystem. Writes replace a complete JSON envelope atomically; newly created Unix directories use `0700` and files `0600`. Callers must serialize concurrent updates to the same entry to avoid lost updates. Raw responses and original Go envelopes are readable, but raw responses never create fictitious job records. Corrupt entries are skipped by listings and rejected by writes.
3. Request keys include every effective option with unambiguous field boundaries. Content fallback is an explicit caller choice. Saving responses preserves previously billed job history. History-only entries do not supply transcripts. Clearing removes only regular SHA-256 JSON files and rejects filesystem-root and home aliases.
4. Preprocessing runs FFmpeg and probes both original and processed durations. Equal, longer or unmeasurable output selects the original and removes temporary output. Byte size does not establish billable duration. The subprocess interface requires UTF-8 paths; other paths fail explicitly when preprocessing is enabled. Disabled preprocessing leaves paths untouched.
5. Missing or pending billing is distinct from a real zero charge. Estimates use published prerecorded list rates, ignore negotiated discounts and return unknown for unpriced models. Unknown response fields survive serialization, including current diarization metadata and confidence values.

# Prerequisites

- A Tokio runtime and a [Deepgram API key](https://developers.deepgram.com/docs/create-additional-api-keys) for network calls.
- FFmpeg and FFprobe when the caller enables preprocessing; cache and Markdown operations need neither.

# Installation

This unpublished implementation crate belongs to the `deepgram-transcribe-cli` repository. From a consumer beside that repository:

```sh
cargo add deepgram-transcription --path ../deepgram-transcribe-cli/crates/deepgram-transcription
```

# Quick Start

```rust
use deepgram_transcription::{RequestOptions, cache::request_key};

let audio = b"recorded audio bytes";
let options = RequestOptions::default();
let key = request_key(audio, &options)?;
println!("Cache identity: {key}");
# Ok::<(), deepgram_transcription::Error>(())
```

# API

| Entry points | Purpose |
| :--- | :--- |
| `Service::new`, `transcribe`, `actual_cost` | Configure an explicit endpoint and key, transcribe binary audio and read billing. |
| `cache::request_key`, `source_key` | Derive complete request and source-content identities. |
| `Cache::envelope`, `response`, `records`, `response_count` | Read real persisted responses and job history. |
| `Cache::save`, `save_response`, `save_record` | Atomically persist an envelope while preserving its other component. |
| `Cache::find_source`, `find_request`, `find_target`, `clear` | Find newest content/history matches and remove owned files. |
| `audio::prepare`, `duration`, `mime_type` | Select audio with a caller-supplied command runner and cancellation token. |
| `cost::estimate` | Calculate a formatted list-price estimate or return unknown. |
| `terms::parse`, `load`, `combine`, `tech_terms` | Read vocabulary and preserve the first spelling when deduplicating. |
| `markdown::format`, `format_seconds` | Render metadata and speaker-grouped Markdown. |

# Configuration

| Option | Default | Meaning |
| :--- | :--- | :--- |
| `RequestOptions` model / language | `nova-3` / `en` | Transcription selection. |
| `RequestOptions.diarize_model` | `latest` | Empty disables speaker labeling. |
| `smart_format`, `utterances`, `punctuate` | `true` | Enabled request features. |
| `Service.timeout` | Five minutes | Includes reading and decoding each response. |
| `PreprocessOptions` | Mono and silence trimming | Threshold `-30dB`, minimum silence `2.0` seconds. |
| `Cache::default_directory()` | `$XDG_CACHE_HOME/deepgram-transcribe` | Falls back to `~/.cache/deepgram-transcribe`, then a relative `.cache` directory. |

Verification uses local HTTP servers, injected subprocess runners and synthetic audio bytes. Live paid requests, actual FFmpeg execution and macOS/Windows runtime behavior remain unverified. [DEPENDENCIES.md](DEPENDENCIES.md) records dependency choices and provenance.

# License

MIT — [LICENSE](./LICENSE).
