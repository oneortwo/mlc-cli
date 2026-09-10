# mlc-cli

An unofficial command-line client for [The MLC Public Search API](https://public-api.themlc.com/api/doc), inspired by [soundcharts-cli](https://github.com/oneortwo/soundcharts-cli). Written in Rust. Binary: `mlc`.

Search musical works and recordings, look up songwriters and publishers, and retrieve works in batches. All four data endpoints in the published API v1.2.0 are supported.

## Install

Download a macOS or Linux binary from [Releases](https://github.com/oneortwo/mlc-cli/releases), or install from source with stable Rust:

```sh
cargo install --git https://github.com/oneortwo/mlc-cli --locked
mlc --help
```

## Authentication

Request Public Search API access through [The MLC](https://www.themlc.com/bulk-database-feed). The API uses a username/password exchange for tokens; bulk-feed credentials are separate. The data endpoints require the returned JWT `idToken` as the bearer token (verified against the live API); the returned `accessToken` is rejected.

Recommended: install the [1Password CLI](https://developer.1password.com/docs/cli/) and save references to your credentials:

```sh
mlc auth setup \
  --username-ref 'op://YOUR_VAULT/YOUR_ITEM/username' \
  --password-ref 'op://YOUR_VAULT/YOUR_ITEM/password'
mlc auth status
mlc doctor
```

Only references are saved in `~/.mlc/config.toml`. `op read` resolves the values at runtime. `auth status` checks configuration without accessing 1Password or authenticating; `doctor` checks live authentication and data access with a small read-only work search, without printing tokens.

Alternatively, inject `MLC_USERNAME` and `MLC_PASSWORD` into the process environment through your secret manager. Both must be set and nonempty. Environment credentials take precedence over 1Password references. There are deliberately no password flags or plaintext credential files.

## Usage

```sh
mlc search works 'Yesterday' --writer-last-name McCartney
mlc search works 'YOUR_TITLE' --writer-ipi YOUR_WRITER_IPI
mlc search recordings --isrc GBAYE0601477
mlc search recordings --title Yesterday --artist 'The Beatles'
mlc work get MLC_SONG_CODE
mlc work batch FIRST_MLC_SONG_CODE SECOND_MLC_SONG_CODE
mlc --json search works 'Yesterday' --writer-last-name McCartney
mlc search works 'Yesterday' --writer-last-name McCartney | jq '.[].mlcSongCode'
mlc completions zsh > _mlc
```

Replace placeholders with actual titles and identifiers. Search criteria are combined in one API request. Work searches require a title and at least one writer field; the live API rejects title-only and writer-only requests, although its schema does not mark those requirements. Work searches support one writer filter per invocation. Use the API's exact identifiers, including leading zeros.

Collections display as tables in a terminal; piped output and `--json` use JSON. Single-work details use formatted JSON to preserve nested writer/publisher ownership chains. Diagnostics go to stderr. Unknown response fields are preserved.

MLC's published API exposes no pagination or total-count parameters. The CLI returns the API response as received; a search is not a guaranteed complete catalog export. Requests time out after 60 seconds. Rate limits are reported without automatic retries. Each command obtains fresh tokens, which are never cached to disk.

## Endpoint coverage

| API | Command |
| --- | --- |
| `POST /oauth/token` | Automatic authentication; `mlc doctor` |
| `POST /search/songcode` | `mlc search works TITLE --writer-last-name NAME` (also supports `--writer-first-name` and `--writer-ipi`) |
| `POST /search/recordings` | `mlc search recordings [--isrc ISRC] [--title TITLE] [--artist ARTIST]` |
| `GET /work/id/{id}` | `mlc work get ID` |
| `POST /works` | `mlc work batch ID...` |

The batch request intentionally uses `mlcsongCode`, matching the API specification's spelling. This tool does not register works, manage claims, or download the bulk database.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success, including empty results |
| 1 | Network, response, input-value, or local I/O error |
| 2 | Authentication/configuration failure, or command-line usage error |
| 3 | HTTP 404 / not found |
| 4 | HTTP 429 / rate limited |

## Security

Credentials and tokens are held in memory only. Requests use HTTPS to the fixed MLC API host and never follow redirects. HTTP error bodies and 1Password stderr are not printed. Known credential/token values are redacted if echoed in successful data responses. Do not put credentials, local config, or real API-response fixtures in Git. See [SECURITY.md](SECURITY.md).

## Development

```sh
make check
make build
make install
```

Tests use synthetic credentials and local mock HTTP servers; no MLC account is required. Tagged versions publish macOS (Apple Silicon and Intel) and Linux (x86-64) archives with SHA-256 checksums through GitHub Actions. To update a source installation, repeat the installation command with `--force`.

MIT licensed. Not affiliated with or endorsed by The MLC. The software license does not grant rights to MLC data; use of the API and data remains subject to [MLC terms](https://themlc.com/musical-works-database-terms-use).
