# mlc-cli

Unofficial Rust CLI for The MLC Public Search API. Binary: `mlc`.

- Run `make check` before publishing changes.
- Keep all HTTP requests in `src/client.rs`.
- Keep credentials and tokens out of source, tests, logs, and Git history.
- Use synthetic fixtures only. Never copy local config into the repository.
- Credentials may be stored in ~/.mlc/config.toml outside the repository. Use owner-only permissions and never print credential values.
- Use the published MLC OpenAPI specification for request field names.
- stdout is data; stderr is diagnostics. Preserve JSON output for pipes.
- Update README endpoint coverage and CHANGELOG for user-facing changes.
