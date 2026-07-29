# Releases

900Slides is built from source — no published installers. This is an
open-source project; no paid Apple Developer Program or code-signing
certificate is used.

## Download a pre-built binary

Every tagged release (`v*`) triggers GitHub Actions builds. Download the
artifacts from the [Actions tab](../../actions):

- **macOS**: `900Slides-macos` — an ad-hoc signed `.app` bundle.
- **Linux**: `900Slides-linux-deb` (`.deb`) and `900Slides-linux-appimage`
  (`.AppImage`).

### macOS: bypassing Gatekeeper

The `.app` is ad-hoc signed (no notarization). On first launch, macOS
Gatekeeper will block it. To open:

1. Right-click the `.app` → **Open** → **Open anyway**.
2. Or from Terminal: `xattr -cr /path/to/900Slides.app`

### Linux

```bash
# .deb
sudo dpkg -i 900Slides_*.deb

# .AppImage
chmod +x 900Slides_*.AppImage
./900Slides_*.AppImage
```

## Build from source

```bash
# Prerequisites: Rust 1.92+, Node.js 22, system deps (see CI workflow)

cd apps/desktop
npm ci
npm run tauri:build
# macOS:  src-tauri/target/release/bundle/macos/900Slides.app
# Linux:  src-tauri/target/release/bundle/deb/ or appimage/
```
