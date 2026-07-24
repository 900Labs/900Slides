# 900Slides

900Slides is a free, local-first desktop presentation editor. It is built for people and communities who need to build, present, and export slides without an account, subscription, telemetry, or constant internet connection.

## Build and run

Prerequisites:

- Rust 1.92.0, pinned by `rust-toolchain.toml`
- Node.js 20.19 or newer, 22.12 or newer, or 24 or newer
- The [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/)

```bash
npm ci --prefix apps/desktop
npm run tauri:dev --prefix apps/desktop
```

Run the local quality gate before committing:

```bash
./scripts/verify-local.sh
```

## License

900Slides is licensed under the Apache License 2.0. See [LICENSE](LICENSE).
