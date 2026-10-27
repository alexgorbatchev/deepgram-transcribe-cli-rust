`deepgram-transcribe` turns audio recordings into speaker-labelled Markdown and keeps transcription history with estimated or billed costs. It serves saved responses by audio contents to avoid repeat paid uploads.

# What It Does

- **Transcribe recordings:** Upload audio with model, language and custom vocabulary choices.
- **Reuse paid results:** Read saved responses offline, including after renaming a recording.
- **Review spending:** List jobs, inspect original request settings and refresh billed charges.
- **Prepare audio:** Downmix or trim silence when FFmpeg produces a recording with a strictly shorter measured duration.

# How It Works

- Supply an audio file and your Deepgram key for a fresh transcription.
- Reuse matching cached audio or prepare and upload the original recording once.
- Receive Markdown on stdout or save it with `--output`; inspect history separately.

# How it Really Works

- Saved responses and job history live under `$XDG_CACHE_HOME/deepgram-transcribe`, falling back to `~/.cache/deepgram-transcribe`. Cache reuse preserves the original request's vocabulary, model and billed record. Legacy raw responses without settings report an unknown model.
- Cached audio matches by original contents even when requested options differ. Use `--force` for a fresh billable request; `--no-cache` bypasses both saved responses and history writes.
- FFmpeg and FFprobe 4.4+ prepare audio under `.tmp/audio_preprocess`. Only a strictly shorter positive measured duration is uploaded; equal, longer or unmeasurable results select original audio. Mono-only processing follows the same rule. Byte size is reported separately, and temporary files are removed.
- Missing or outdated programs warn and upload original audio. Processing failures stop the command. Cache-write failures warn while still delivering the transcript.
- Audio uploads make one attempt without redirects. Interrupting a request does not prove Deepgram avoided billing. History refresh looks up pending charges with at most eight concurrent requests; failed lookups remain pending.
- Markdown is identical in both output modes. Progress and returned errors go to stderr; `AGENT=1` selects compact TSV and key/value history output. Printed paths fold home to `~`. Exit 0 means success, 1 means failure and 130 means cancellation.
- `job inspect` reports whether preprocessing was requested; that field does not claim preprocessing ran. `cache status` counts reusable responses, excluding history-only entries. `cache clear` removes owned responses and history while retaining unrelated files.

# Installation

The Rust executable is local and has no published release. No Rust download artifact or installation URL is available yet.

# Setup

- [Deepgram](https://developers.deepgram.com/docs/create-additional-api-keys) — set `DEEPGRAM_API_KEY` or pass `--api-key` for fresh transcription and billing refresh. Cached transcription and offline history need no credentials.
- [FFmpeg](https://ffmpeg.org/download.html) — FFmpeg and FFprobe 4.4+ are optional for preprocessing. Use `dependency list` to check them or explicit `dependency install` to invoke your host package manager.

# Quick Start

```sh
deepgram-transcribe transcript create interview.wav -o interview.md
deepgram-transcribe transcript create interview.wav --force -t 'Envoy,Rust'
AGENT=1 deepgram-transcribe job list --limit 10
deepgram-transcribe job inspect interview.wav
AGENT=1 deepgram-transcribe cache status
```

Sample Output for an empty cache, as verified by the native process fixture:

```text
cache_directory: ~/.cache/deepgram-transcribe
cached_responses: 0
```

# Options & Flags

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--api-key <key>` | — | Empty; `DEEPGRAM_API_KEY` fallback | Credentials for fresh requests and billing lookup. |
| `--cache-dir <path>` | — | XDG/home cache path | Saved responses and history. |
| `--version` | `-v` | `false` | Print raw version plus newline. |
| `--help` | `-h` | `false` | Show help offline. |

`deepgram-transcribe transcript create <audio-file>`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--term <text>` | `-t` | Unset | Repeat or comma separate extra vocabulary. |
| `--terms-file <path>` | — | Unset | Vocabulary file; one term per line. |
| `--model <name>` | `-m` | `nova-3` | Deepgram model. |
| `--language <code>` | `-l` | `en` | Recording language. |
| `--output <path>` | `-o` | Unset; stdout | Create parents and overwrite Markdown. |
| `--force` | `-f` | `false` | Bypass cache reads for another paid request. |
| `--no-diarize` | — | `false` | Disable speaker labeling. |
| `--no-tech-terms` | — | `false` | Omit built-in engineering vocabulary. |
| `--no-cache` | — | `false` | Bypass all cache reads and writes. |
| `--no-preprocess` | — | `false` | Upload original audio. |
| `--no-mono` | — | `false` | Skip downmixing. |
| `--no-trim-silence` | — | `false` | Skip silence trimming. |
| `--silence-threshold <value>` | — | `-30dB` | FFmpeg amplitude/decibel threshold. |
| `--silence-duration <seconds>` | — | `2.0` | FFmpeg silence duration. |

`deepgram-transcribe job list`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--limit <number>` | — | `0` | Unsigned recent-job limit; zero lists all. |

`deepgram-transcribe completion <bash|zsh|fish|powershell>`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--no-descriptions` | — | `false` | Omit descriptions from generated completions. |

Read `AGENT=1 deepgram-transcribe skill` for the complete embedded command reference, including history, cache and dependency actions.

# License

[MIT](LICENSE), copyright Alex Gorbatchev.
