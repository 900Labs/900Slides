# Security Policy

## Supported versions

900Slides is pre-1.0 software. Only the latest release receives security
fixes.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a vulnerability

If you discover a security vulnerability, please report it privately:

1. Do **not** open a public GitHub issue.
2. Email **security@900labs.com** with a description and reproduction steps.
3. You will receive an acknowledgement within 72 hours.

We request that you give us 90 days to address the issue before public
disclosure.

## Threat model

900Slides is a local-first desktop application with no network calls, no
accounts, and no telemetry. The primary attack surface is the file-format
boundary: PPTX and other imported files are untrusted input.

- Imported archives (PPTX, ODP) are validated against a schema. ZIP entries
  are size-capped (50 MiB per entry, 500 MiB total) and checked for path
  traversal.
- Embedded media is MIME-allowlisted. EXIF metadata is stripped on import
  unless explicitly preserved.
- Opaque payloads (OLE objects, unsupported embedded media) are dropped with
  a warning rather than executed or rendered.

See `PRODUCT_SPEC.md` section 7.4 for the format-security details.

## Privacy

- No telemetry, analytics, or crash reporting.
- No account, login, or token storage.
- Recent files are stored as privacy-preserving salted hashes of the path.
- The app data directory is the only location for recovery snapshots and
  settings.

Run the public-release privacy gate to verify no local paths, hostnames, or
secrets appear in fixtures or logs:

```bash
./scripts/verify-public-release.sh
```
