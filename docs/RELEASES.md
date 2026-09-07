# Building and qualifying releases

The source is on the 0.4 development track. The release workflow runs on `v*`
tags and manual dispatch. It uploads Actions artifacts; it does not automatically
publish GitHub Release assets. See successful runs in the
[release workflow](https://github.com/900Labs/900Slides/actions/workflows/release.yml).

## Artifact configuration

| Platform | Artifact | Package command |
| --- | --- | --- |
| macOS | `900Slides-macos`, ad-hoc signed `.app` | `npm run tauri:build --prefix apps/desktop -- --bundles app` |
| Linux | `900Slides-linux-deb`, `900Slides-linux-appimage` | `npm run tauri:build --prefix apps/desktop -- --bundles deb,appimage` |
| Windows | `900Slides-windows-offline`, NSIS `.exe` | `npm run tauri:build --prefix apps/desktop -- --bundles nsis` |

Platform-specific Tauri configuration selects the same bundle targets for a
plain `npm run tauri:build --prefix apps/desktop` invocation.

Build on the target platform after installing Rust 1.92.0, a supported Node.js
version (see README), the Tauri prerequisites, and npm dependencies. End users
installing a qualified package do not need these development tools.

Linux packages are built on Ubuntu 22.04 to reduce accidental glibc requirements.
This is a build baseline, not confirmation that every Linux distribution works.
Windows embeds the offline WebView2 installer; the build needs network access
to obtain it, while installation is intended to work without network access.
This runtime adds substantial size. The configuration follows
[Tauri's installer guidance](https://v2.tauri.app/distribute/windows-installer/).

The `.app` is ad-hoc signed and is not notarized. Use macOS's documented
[Open Anyway flow](https://support.apple.com/en-us/102445) only for a build whose
origin you have verified. The Windows installer is not code signed; installer
warnings and standard-user behavior need clean-machine validation.

For Linux, a `.deb` may need additional installed system packages; download them
in advance for offline use. An AppImage may require FUSE or its documented
extract-and-run option. See [Tauri AppImage guidance](https://v2.tauri.app/distribute/appimage/).

## Required release evidence

1. Run `./scripts/verify-local.sh` and the native development smoke test in
   `AGENTS.md`. CI also builds the production desktop protocol on all platforms.
2. Build fresh platform packages. Missing artifacts fail the release jobs.
3. Run `./scripts/verify-wave22.sh` on the packaged executable. Its 16 MiB budget
   covers executable bytes, not RAM, webviews, downloads or total installation.
4. Record artifact checksums and sizes, supported OS/runtime/architecture,
   clean installation, offline first run and recovery results.
5. Qualify the device/workload checks in
   [LOW_RESOURCE_REQUIREMENTS.md](LOW_RESOURCE_REQUIREMENTS.md). Keep automated
   tests, local smoke tests and target-device qualification separate.

Changing a workflow or building on one development machine does not qualify
Windows/Linux installation or 2 GB/4 GB usability. Current verification and
remaining blockers are recorded in [Wave 23](sprint-records/wave-23.md).
