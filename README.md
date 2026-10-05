# ProctorLAN

Offline classroom exam proctoring over a local network. The teacher's computer hosts; student computers join with a session code. No internet, no cloud, no telemetry.

**Status:** Phase 8 of 13 — synchronization done (offline answer queue saved to disk, batch upload, takeover of stale connections, flood protection). See `docs/synchronization.md`.

## Requirements
- Node.js 20+ and npm
- Rust 1.90+ (via [rustup](https://rustup.rs))
- Linux only: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev build-essential pkg-config libxdo-dev`
- Windows: WebView2 (preinstalled on Win 10/11) and MSVC build tools. macOS: Xcode command line tools.

## Commands
```bash
npm install
npm run dev          # UI only in a browser (Rust bridge unavailable)
npm run tauri:dev    # full desktop app
npm test             # frontend tests
npm run typecheck
cd src-tauri && cargo test
npm run tauri:build  # installers for the current OS
```
Installers are built per-OS; you cannot cross-build a macOS .dmg from Windows/Linux.
