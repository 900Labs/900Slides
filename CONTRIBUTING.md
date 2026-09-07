# Contributing to 900Slides

Thank you for your interest in contributing to 900Slides. This guide covers
how to set up a development environment, run the quality gate, and open a
pull request.

## Prerequisites

- Rust 1.92.0, pinned by `rust-toolchain.toml`
- Node.js 20.19 or newer, 22.12 or newer, or 24 or newer
- The [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/)

## Getting started

```bash
git clone https://github.com/900Labs/900Slides.git
cd 900Slides
npm ci --prefix apps/desktop
npm run tauri:dev --prefix apps/desktop
```

## Quality gate

Run the full local gate before opening a pull request. Every check must pass:

```bash
./scripts/verify-local.sh
```

The automated gate checks the 11-crate workspace (formatting, clippy, tests),
then the frontend (typecheck, unit tests, production build), then the separate
Tauri crate (formatting, clippy, tests), and finally the public-release gate.
Install dependencies with `npm ci --prefix apps/desktop` before running it.

Complete the required native smoke test after the automated gate:

```bash
npm run tauri:dev --prefix apps/desktop
```

Follow [`docs/LOW_RESOURCE_REQUIREMENTS.md`](docs/LOW_RESOURCE_REQUIREMENTS.md)
for small-screen, offline, recovery, and constrained-device acceptance checks.
A fast development machine or a small executable does not establish the minimum
hardware requirement. CI builds the production desktop protocol on all three
platforms; package installation still requires a platform smoke test.

If a command fails, fix the issue and re-run from the top. Do not open a pull
request with a failing gate.

## Project conventions

- **No telemetry, analytics, or remote calls** are permitted in the
  application code. Do not add network dependencies.
- The **workspace contains only the 11 library crates** under `crates/`. The
  Tauri v2 desktop app lives in `apps/desktop/` and is built separately.
- `slides-core` is the canonical source of truth for the deck model. The
  desktop frontend is a read-and-command projection — it never owns state as
  truth. See `PRODUCT_SPEC.md` section 6.6.
- PPTX is the native format. Save paths must preserve untouched parts
  byte-for-byte. See `PRODUCT_SPEC.md` section 7.1.
- Every public function and struct must have a doc comment (Rust) or JSDoc
  (TypeScript).
- No comments in code unless they explain a non-obvious decision.

## Pull request checklist

- [ ] The quality gate passes locally.
- [ ] New or changed behavior has a test.
- [ ] Public claims about file-format compatibility are backed by a passing
      test against a generated or sanitized fixture.
- [ ] Documentation is updated if behavior, workflows, or public APIs change.
- [ ] No secrets, local paths, hostnames, or telemetry are introduced.

## Compatibility fixtures

Compatibility claims (PPTX round-trip, ODP import, etc.) must be backed by
generated or sanitized placeholder fixtures under `crates/slides-fixtures/`.
Do not commit files exported from proprietary software. Invent content for
test fixtures.

## Reporting issues

Use [GitHub Issues](https://github.com/900Labs/900Slides/issues) for bug
reports and feature requests. For security vulnerabilities, see
[SECURITY.md](SECURITY.md) — do not open a public issue.
