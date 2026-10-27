mod deepgram_transcription 'crates/deepgram-transcription/justfile'

run *args:
    cargo run --package deepgram-transcribe-cli --locked --bin deepgram-transcribe -- {{ args }}

run-ai *args:
    AGENT=1 cargo run --package deepgram-transcribe-cli --locked --bin deepgram-transcribe -- {{ args }}

build:
    cargo build --package deepgram-transcribe-cli --locked --release --bin deepgram-transcribe
    mkdir -p bin
    cp target/release/deepgram-transcribe bin/deepgram-transcribe

[private]
_test:
    mkdir -p .tmp
    TMPDIR="$PWD/.tmp" cargo test --package deepgram-transcribe-cli --locked --all-targets

[private]
_lint:
    cargo fmt --package deepgram-transcribe-cli -- --check
    cargo clippy --package deepgram-transcribe-cli --locked --all-targets -- -D warnings

coverage:
    mkdir -p .tmp
    TMPDIR="$PWD/.tmp" cargo llvm-cov --package deepgram-transcribe-cli --exclude-from-report deepgram-transcription --locked --all-targets --fail-under-lines 90

doc:
    RUSTDOCFLAGS='-D warnings' cargo doc --package deepgram-transcribe-cli --locked --no-deps

check: _lint _test coverage doc
    just deepgram_transcription check

[private]
_fmt:
    cargo fmt --package deepgram-transcribe-cli -- --check

test: _test
    just deepgram_transcription test

lint: _lint
    just deepgram_transcription lint

fmt: _fmt
    just deepgram_transcription fmt-check
