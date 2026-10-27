---
name: deepgram-transcribe
description: Use when operating deepgram-transcribe to transcribe audio, inspect transcription costs, manage cached responses, or manage ffmpeg.
author: alexgorbatchev
metadata:
  created_on: 2026-10-01 21:03
  last_modified: 2026-10-14 19:12
  status: current
---

## Workflows

Read this reference with `AGENT=1 deepgram-transcribe skill`.

```bash
export DEEPGRAM_API_KEY="your-deepgram-api-key"
deepgram-transcribe transcript create interview.opus > interview.md
deepgram-transcribe transcript create interview.m4a -t Envoy -t 'Alex,Harvey' -o notes.md
deepgram-transcribe job list --limit 10
deepgram-transcribe job inspect interview.m4a
deepgram-transcribe cache status
deepgram-transcribe dependency list
```

Use `--force` to request a fresh, billable transcription with changed options.
Use `--no-cache` to bypass cached responses and leave response/history storage untouched.
Use `--no-preprocess` to upload the original audio. Use `--no-mono` or
`--no-trim-silence` to select which processing steps run.

## Output, environment and errors

- Transcription outputs Markdown with a metadata table and timestamped speaker turns, identically in both modes. `--output` writes that result to a file, creates parent directories and overwrites an existing file.
- Progress, confirmations, warnings and errors go to stderr. Help, completion scripts and `skill` go to stdout. Syntax-error usage goes to stderr. Success exits 0; a command error exits 1; cancellation exits 130. Each returned runtime error prints once without usage.
- `AGENT=1`, `true` or `yes` selects compact output; matching ignores case and surrounding whitespace. Other values select human output. Listings use TSV, details use `key: value` lines, and diagnostics use `INFO:`, `WARN:`, `OK:`, `ERR:` or `HINT:`. Human output uses bordered tables, aligned fields and bracketed tags. Agent help begins with the skill-reading alert and retains full descriptions.
- `COLUMNS` accepts a positive integer for human help/output width. Otherwise terminal width is detected; human help and output fall back to 100 columns when unavailable. Agent help is untruncated.
- `DEEPGRAM_API_KEY` supplies credentials when `--api-key` is empty. A nonempty flag takes precedence. A cache hit works without credentials; a fresh request needs credentials. History commands query the management API for pending charges when credentials are available and save resolved charges locally.
- An empty `--cache-dir` resolves to `$XDG_CACHE_HOME/deepgram-transcribe`, then `~/.cache/deepgram-transcribe`; if the home directory is unavailable, it uses `.cache/deepgram-transcribe` relative to the working directory.
- `$XDG_DATA_HOME/deepgram-transcribe/bin` (or `~/.local/share/deepgram-transcribe/bin`; Windows uses `LOCALAPPDATA` before home fallback) is prepended to child-process `PATH` for managed-program discovery. The parent environment is preserved. The `dependency install` and `dependency update` commands use the host package manager and can change packages outside that directory.
- Preprocessing uses ffmpeg 4.4 or newer and creates temporary audio in `.tmp/audio_preprocess` under the working directory. ffprobe measures the first audio stream's duration in both recordings. Upload prepared audio only when its duration is strictly shorter; equal, longer or unavailable durations select original audio and immediately remove the prepared file. This also applies to mono-only processing. Selected temporary audio is removed after the run. Progress reports duration and byte comparisons; re-encoding can increase bytes despite shorter duration. Deepgram's response duration and channel count determine the estimated charge.
- Missing or outdated ffmpeg or ffprobe emits a warning and uploads original audio. An error while processing with usable ffmpeg fails the command. Duration-probe errors warn and select original audio; cancellation fails the command. File-read, terms-file, API, and transcript-write errors fail the command. Cache/history write failures warn while allowing transcript delivery. Management-cost lookup failures leave charges pending, with details in agent mode.
- Cached results are matched by original audio contents, including requests with different options. A cached response can therefore retain an earlier model, vocabulary or speaker configuration. `--force` skips reads and updates storage; `--no-cache` skips both reads and writes. `cache clear` deletes responses and job history in the selected cache directory.
- Serving a cached transcript preserves its original history and billed cost, including when requested options differ. History-only entries contain no reusable response and are ignored as transcript hits; lookup can still find another saved response for the same audio.
- Uploads go to Deepgram's pre-recorded transcription API. Keyterms, model, language and formatting options are sent with the audio. A cancelled run cancels the in-flight request; a completed server-side transcription may already be billable.
- Uploads have one attempt and reject redirects; a transport error can leave a server-side charge uncertain. Billing lookups use at most eight concurrent requests. Cancellation stops local work and preserves existing cache records.
- Saved Markdown describes the cached request's original model and vocabulary. Legacy raw responses without history use unknown model metadata. Cache status counts actual responses, excluding history-only entries. Printed paths fold home to `~`; saved data retains original paths.

## Command and flag reference

Tables list parser defaults exactly. An empty string is shown as an empty pair
of backticks; effective defaults are described above or in the behavior column.
Domain and metadata commands accept inherited `--api-key` and `--cache-dir`. `--help`/`-h` displays their help.
Generated `help [command]...` forms take a command path without flags. `--version`/`-v` is available on the root only.
Booleans accept `--flag` and `--flag=true`/`--flag=false`. Root version and help are presence flags. String flags take a value;
short aliases accept the same value. Domain verbs without a positional argument
reject extra arguments. Subject groups display their help when run alone.

### `deepgram-transcribe`

Invocation: `deepgram-transcribe`

Display root help, invoke a subject command, or print the bare version with `--version`.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--api-key` | `` | `string` | empty | Credentials; effective default is `DEEPGRAM_API_KEY` |
| `--cache-dir` | `` | `path` | unset | Cache directory; effective default follows XDG/home resolution above |
| `--help` | `h` | `bool` | `false` | Print requested help |
| `--version` | `v` | `bool` | `false` | Print the bare version and exit |

### `deepgram-transcribe skill`

Invocation: `deepgram-transcribe skill`

Print this entire embedded Markdown reference verbatim, including frontmatter.
Accepts zero positional arguments and has no command-specific flags. Runs offline
without credentials or repository files. Output-write failures fail the command.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print command help |

### `deepgram-transcribe transcript`

Invocation: `deepgram-transcribe transcript`

Display the transcript command subtree.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print subject help |

### `deepgram-transcribe transcript create`

Invocation: `deepgram-transcribe transcript create <audio-file>`

Supply exactly one `<audio-file>` path. The file is read locally; extension-based
MIME detection recognizes mp3, m4a, mp4, wav, flac, ogg and aac. Other extensions,
including opus, are uploaded as `application/octet-stream` for Deepgram to detect.
Deepgram determines whether the actual audio is supported.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print command help |
| `--term` | `t` | repeated `string` | unset | Repeat or use comma-separated keyterms; whitespace and empty terms are removed |
| `--terms-file` | `` | `path` | unset | Additional keyterms, one per line; blank and comment lines are skipped |
| `--model` | `-m` | `string` | `nova-3` | Model name passed to Deepgram; accepted names are determined by the API |
| `--language` | `-l` | `string` | `en` | Language identifier passed to Deepgram; accepted identifiers are determined by the API |
| `--output` | `o` | `path` | unset | Omitted or empty means stdout; a path creates/overwrites a Markdown file |
| `--force` | `-f` | `bool` | `false` | Bypass cache reads and request a billable transcription |
| `--no-diarize` | `` | `bool` | `false` | Disable speaker labeling in the API request |
| `--no-tech-terms` | `` | `bool` | `false` | Exclude built-in engineering keyterms |
| `--no-cache` | `` | `bool` | `false` | Bypass cache reads and writes, including job history |
| `--no-preprocess` | `` | `bool` | `false` | Disable both processing steps |
| `--no-mono` | `` | `bool` | `false` | Retain channel count during preprocessing |
| `--no-trim-silence` | `` | `bool` | `false` | Retain silence during preprocessing |
| `--silence-threshold` | `` | `string` | `-30dB` | ffmpeg silence threshold; accepts ffmpeg amplitude or decibel syntax |
| `--silence-duration` | `` | `string` | `2.0` | ffmpeg silence duration in seconds; ffmpeg validates its value |

### `deepgram-transcribe job`

Invocation: `deepgram-transcribe job`

Display the history command subtree.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print subject help |

### `deepgram-transcribe job list`

Invocation: `deepgram-transcribe job list`

Accepts zero positional arguments. List recent jobs and totals for the displayed
jobs. Output columns are date, file, duration, channels, model, cost and request ID.
Unresolved charges show `not billed yet`; an empty history is reported on stderr.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print command help |
| `--limit` | `` | `uint32` | `0` | Positive values limit recent jobs; zero lists all; negative values fail validation |

### `deepgram-transcribe job inspect`

Invocation: `deepgram-transcribe job inspect <audio-file|request-id>`

Supply exactly one `<audio-file|request-id>`. Match a readable audio file by its
contents or look up a request ID from history. Print original file/path, request
ID, timestamp, duration, channels, model, preprocessing, speaker configuration,
keyterms and estimated/actual cost. `Preprocessing requested` (agent key
`preprocessing_requested`) reflects the stored request's processing options;
it does not assert that preparation ran or reduced bytes. A missing match fails
with a history hint.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print command help |

### `deepgram-transcribe cache`

Invocation: `deepgram-transcribe cache`

Display the cache command subtree.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print subject help |

### `deepgram-transcribe cache status`

Invocation: `deepgram-transcribe cache status`

Accepts zero positional arguments. Print the selected cache directory and number
of reusable cached responses, excluding history-only entries.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print command help |

### `deepgram-transcribe cache clear`

Invocation: `deepgram-transcribe cache clear`

Accepts zero positional arguments. Delete cached JSON responses and history
without prompting; report the number removed on stderr. Other files are retained.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print command help |

### `deepgram-transcribe dependency`

Invocation: `deepgram-transcribe dependency`

Display the external-program command subtree.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print subject help |

### `deepgram-transcribe dependency list`

Invocation: `deepgram-transcribe dependency list`

Accepts zero positional arguments. Execute ffmpeg and ffprobe version queries and list program,
minimum version (4.4), installed version and status. Missing/outdated tools are
shown as status information; it does not cause a command error.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print command help |

### `deepgram-transcribe dependency install`

Invocation: `deepgram-transcribe dependency install`

Accepts zero positional arguments. Install missing or outdated ffmpeg or ffprobe through their shared ffmpeg package using
Homebrew, apt-get, pacman, dnf, winget or Chocolatey, according to host support.
Already satisfied dependencies are skipped. Package-manager permissions apply.
Failure returns an error and suggests manual installation.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print command help |

### `deepgram-transcribe dependency update`

Invocation: `deepgram-transcribe dependency update`

Accepts zero positional arguments. Invoke the host package manager's installation
operation for every declared program, including installed ffmpeg and ffprobe. Whether that
operation upgrades an existing package depends on the host package manager.
Failure returns an error and suggests manual updating.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print command help |

### `deepgram-transcribe help`

Invocation: `deepgram-transcribe help [command]`

Supply an optional command path, such as `help transcript create`. Print help for
that path; an unknown target fails argument validation.

Root generated paths: `deepgram-transcribe help transcript`, `deepgram-transcribe help transcript create`,
`deepgram-transcribe help job`, `deepgram-transcribe help job list`, `deepgram-transcribe help job inspect`,
`deepgram-transcribe help cache`, `deepgram-transcribe help cache status`, `deepgram-transcribe help cache clear`,
`deepgram-transcribe help dependency`, `deepgram-transcribe help dependency list`, `deepgram-transcribe help dependency install`, `deepgram-transcribe help dependency update`,
`deepgram-transcribe help completion`, `deepgram-transcribe help completion bash`, `deepgram-transcribe help completion zsh`, `deepgram-transcribe help completion fish`, `deepgram-transcribe help completion powershell`,
`deepgram-transcribe help skill`, and `deepgram-transcribe help help`.

Subject-local generated paths: `deepgram-transcribe transcript help`, `deepgram-transcribe transcript help create`, `deepgram-transcribe transcript help help`;
`deepgram-transcribe job help`, `deepgram-transcribe job help list`, `deepgram-transcribe job help inspect`, `deepgram-transcribe job help help`;
`deepgram-transcribe cache help`, `deepgram-transcribe cache help status`, `deepgram-transcribe cache help clear`, `deepgram-transcribe cache help help`;
`deepgram-transcribe dependency help`, `deepgram-transcribe dependency help list`, `deepgram-transcribe dependency help install`, `deepgram-transcribe dependency help update`, `deepgram-transcribe dependency help help`;
`deepgram-transcribe completion help`, `deepgram-transcribe completion help bash`, `deepgram-transcribe completion help zsh`, `deepgram-transcribe completion help fish`, `deepgram-transcribe completion help powershell`, `deepgram-transcribe completion help help`.

Use `deepgram-transcribe help help` to display help-command usage.

### `deepgram-transcribe completion`

Invocation: `deepgram-transcribe completion`

Display shell completion choices. Accepts zero positional arguments.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print completion help |

### `deepgram-transcribe completion bash`

Invocation: `deepgram-transcribe completion bash`

Accepts zero positional arguments. Print a Bash completion script to stdout;
loading it requires bash-completion. Save or source the script to activate it.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print shell help |
| `--no-descriptions` | `` | `bool` | `false` | Generate completions without descriptions |

### `deepgram-transcribe completion zsh`

Invocation: `deepgram-transcribe completion zsh`

Accepts zero positional arguments. Print a Zsh completion script to stdout. Load
completions with `compinit`, then source the script or save it on your `fpath`.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print shell help |
| `--no-descriptions` | `` | `bool` | `false` | Generate completions without descriptions |

### `deepgram-transcribe completion fish`

Invocation: `deepgram-transcribe completion fish`

Accepts zero positional arguments. Print a Fish completion script to stdout;
source it or save it in Fish's completions directory.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print shell help |
| `--no-descriptions` | `` | `bool` | `false` | Generate completions without descriptions |

### `deepgram-transcribe completion powershell`

Invocation: `deepgram-transcribe completion powershell`

Accepts zero positional arguments. Print a PowerShell completion script to stdout;
pipe it to `Out-String | Invoke-Expression` or add it to your PowerShell profile.

| Flag | Short | Type | Parser default | Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `--help` | `h` | `bool` | `false` | Print shell help |
| `--no-descriptions` | `` | `bool` | `false` | Generate completions without descriptions |
