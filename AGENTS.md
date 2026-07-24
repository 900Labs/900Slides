# Agent Notes for 900Slides

This repository uses the exact lint, typecheck, and test commands from the project scaffolding. Run them in this order:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check --prefix apps/desktop
npm run tauri:dev --prefix apps/desktop
```

- No telemetry, analytics, or remote calls are permitted in the application code.
- The workspace contains only the 11 library crates under `crates/`.
- The Tauri v2 desktop app lives in `apps/desktop/` and is built separately from the workspace.
