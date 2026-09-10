# mlc-cli

Unofficial Rust CLI for The MLC Public Search API. Binary: `mlc`.

- Run `make check` before publishing changes.
- Keep all HTTP requests in `src/client.rs`.
- Keep credentials and tokens out of source, tests, logs, and Git history.
- Use synthetic fixtures only. Never copy local config into the repository.
- Credentials live in ~/.mlc/config.toml outside the repository. Keep owner-only permissions and never print credential values. `MLC_API_URL` points tests at a mock server; never hit the live API in tests.
- Use the published MLC OpenAPI specification for request field names.
- stdout is data; stderr is diagnostics. Preserve JSON output for pipes.
- Update README endpoint coverage and CHANGELOG for user-facing changes.
