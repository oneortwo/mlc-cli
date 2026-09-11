# Changelog

## Unreleased

- Add `mlc update` to install the latest GitHub release without MLC credentials. Supports all four release platforms, requires a matching SHA-256 checksum, avoids downgrades, and replaces the executable atomically. Refreshes existing shell completions and displays release notes on stderr, preserving JSON output for pipes.

## 0.2.0

- `mlc auth setup` prompts for the username and password, verifies them against the API, and only then saves them. `--no-input` reads `MLC_USERNAME` and `MLC_PASSWORD` for scripts and agents.
- Add a `curl | sh` installer that downloads a release binary, verifies its checksum, and installs shell completions. No Rust toolchain needed.
- Publish Linux arm64 binaries alongside macOS and Linux x86-64.
- Remove the leftover 1Password integration. A config file from 0.1.x that still holds `op://` references now asks you to run `mlc auth setup` instead of failing to parse.
- Only tokens are scrubbed from API responses. Titles that happened to contain your username or password were previously replaced with `[REDACTED]`.
- `MLC_API_URL` overrides the API host for tests and proxies.

## 0.1.1

- Save credentials locally with owner-only permissions.
- Import existing credential references once during setup instead of resolving them on every run.

## 0.1.0

- Search works by title/writer and recordings by ISRC/title/artist.
- Retrieve individual works and batches with writer/publisher details.
- Load credentials from 1Password references or environment variables.
- Add authentication diagnostics, JSON/table output, and shell completions.
- Publish macOS and Linux binaries with checksums.
