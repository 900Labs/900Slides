# Wave 22 — startup, editing, and release optimization

Status: Complete
Owner: 900 Labs
Last updated: 2026-08-03

## Goal

Reduce release size and repeated editing work without weakening the local-first
data model, recovery guarantees, or renderer correctness. The application
remains fully offline: this wave adds no telemetry, analytics, remote calls, or
external assets.

## Baseline

The pre-wave release binary measured approximately 30 MB. A manual optimized
profile experiment measured 14,066,400 bytes, establishing a 16 MiB regression
budget for the maintained release configuration.

## Delivered changes

| Area | Result |
| --- | --- |
| Release profile | The standalone Tauri workspace now owns an explicit size profile: `opt-level = "s"`, LTO, one codegen unit, symbol stripping, and abort-on-panic. |
| Dependencies | Image decoding is limited to PNG, JPEG, GIF, and WebP; ZIP defaults are disabled in favor of DEFLATE support; the direct `dirs` dependency matches Tauri's major version. |
| Recovery | Editing schedules one worker per active burst. PPTX serialization begins only after 750 ms of inactivity, and token/deck checks prevent canceled, switched, or manually saved generations from being written. Atomic file replacement is unchanged. |
| Spell checking | The bundled dictionary and persisted user words initialize on the first spell command rather than during application startup. |
| Media IPC | The core media store owns a cheap mutation generation that changes on every effective insert, same-key replacement, removal, and deserialize. Unchanged snapshots omit base64 media and carry a monotonic revision; the frontend restores only an exact deck/revision match and can request a full snapshot after a webview reload. |
| Thumbnails | `IntersectionObserver` limits SVG rendering to visible or near-visible slides. An LRU cache keys full slide content plus a single deck-wide render signature covering theme/background/high contrast, template, layouts, master, slide size, and media revision. |

## Correctness coverage

- Recovery tracker tests cover one-worker bursts, the 750 ms idle boundary,
  newest-generation selection, and cancellation used by deck switching and
  manual saves.
- Media tests cover the first full payload, unchanged omission, insert/remove
  revision changes, and forced full resynchronization.
- Frontend tests reject cross-deck and cross-revision media reuse.
- Thumbnail tests prove invalidation for text, style, theme, background,
  high-contrast mode, template, layouts, master layers, slide size, and media.
- Spell tests prove the checker remains uninitialized until first use and then
  loads persisted user words.

## Compilation-artifact sharing decision

The root libraries and Tauri application intentionally remain separate Cargo
workspaces and therefore use separate target directories by default. Cargo can
share a manually selected target directory, but committing one shared target
for both workspaces would introduce lock contention during parallel checks and
platform-specific cache churn. Wave 22 leaves this as a developer opt-in rather
than changing reliable cross-platform defaults.

## Verification

The reproducible size gate is:

```bash
./scripts/verify-wave22.sh
```

With no path argument, the script runs the full Tauri release pipeline. This
rebuilds production frontend assets through `beforeBuildCommand`, enables the
distributable `custom-protocol` feature, creates the platform bundle, and then
measures the target executable. A packaged executable can be checked without a
rebuild by passing its path explicitly. No-argument mode requires installed npm
dependencies and the platform's Tauri system prerequisites. The representative
macOS release measured as follows:

- Custom-protocol target executable: 12,973,264 bytes.
- Signed app executable: 12,915,984 bytes.
- Total files in the `.app` bundle: 12,919,118 bytes.
- Production frontend: 198.40 kB JavaScript and 45.59 kB CSS before gzip.
- Standalone Tauri tests: 18 passed.
- Frontend unit tests: 21 passed.
- Svelte/TypeScript check: 0 errors and 0 warnings.
- Public-release privacy gate: passed.

Platform binaries and packaging overhead may differ in size; the script
enforces the same 16 MiB executable budget on the current platform.
