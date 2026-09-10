# Security

Never submit credentials or access tokens in issues, pull requests, fixtures, screenshots, or logs. For a vulnerability, use GitHub's private vulnerability reporting on this repository.

Credentials are accepted only from environment variables or runtime 1Password reads. Configuration contains references only. No tokens are persisted. The client does not follow redirects and does not print remote error bodies. Release builds use no credentials.

The public tests contain invented values only. Before publishing changes, inspect the full staged diff and scan for secrets. If a credential is accidentally disclosed, revoke it immediately; deleting a file does not remove it from Git history.
