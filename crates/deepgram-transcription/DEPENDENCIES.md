# Provenance and dependency choices

Source: Alex Gorbatchev's MIT-licensed [deepgram-transcribe-cli](https://github.com/alexgorbatchev/deepgram-transcribe-cli), inspected under the migration parent's `.tmp/sources/`. This library separates its reusable audio, cache, request, cost, terms and report functionality from the CLI.

| Go dependency or functionality | Rust choice | Reason |
| :--- | :--- | :--- |
| `net/http` | Reqwest with Rustls | Native typed requests, sensitive authentication headers, cancellation, explicit disabled redirects and retries. |
| Deepgram SDK candidate | Direct native HTTP client | SDK 0.13 lacks `diarize_model`, requires fields absent from original cache responses and does not expose the redirect policy needed to prevent replaying paid uploads. Its default client authentication does not travel with a built request executed through a different client. |
| JSON and current API records | Serde / Serde JSON | Typed fields plus lossless unknown-field maps; optional billing fields remain genuinely optional. |
| SHA-256 request identities | SHA2 | Structured complete request keys, distinct source-content keys. |
| Cache and audio transactions | Tempfile | Private same-directory publication and owned temporary-file cleanup. |
| External commands and dependencies | Sibling `cli-deps` | Existing typed command runner and cancellation contract. |
| Dates and cache location | Chrono / Dirs | UTC timestamps and native home discovery with explicit XDG preference. |

Current behavior follows Deepgram's [diarization contract](https://developers.deepgram.com/docs/diarization), [prerecorded API](https://developers.deepgram.com/reference/speech-to-text/listen-pre-recorded) and [published pricing](https://deepgram.com/pricing), inspected on 2026-10-14. Nova-3 monolingual costs $0.0043/minute/channel, multilingual $0.0052, Whisper $0.0048, with $0.0013 for keyterm prompting. Smart formatting and prerecorded diarization are included. Estimates are not proof of a billed charge.

The source's incomplete concatenated request identity is replaced with structured complete options. Cache status can count actual responses separately from history-only entries. Unknown speakers stay unknown. Original billable history is preserved, corrupt cache data is not silently overwritten and root/home symlink aliases cannot be cleared. UTF-8 preprocessing paths are checked rather than silently rewritten.
