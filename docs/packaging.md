# Packaging and releases (Phase 12)

## What you get
| Platform | Installers | Built on |
|---|---|---|
| Windows 10/11 | `.exe` (NSIS setup) and `.msi` | a Windows machine (or the GitHub Actions Windows runner) |
| macOS 10.15+ | `.dmg` (universal: Intel + Apple Silicon) and `.app` | a Mac (or the macOS runner) |
| Linux | `.deb`, `.rpm`, `.AppImage` | Linux (Ubuntu 22.04 runner recommended) |

Tauri cannot cross-compile a Windows or macOS installer from another system, so each platform is built on its own system. The easy way is GitHub Actions (below).

## Building on your own computer
```
npm install
npm run release:check      # type check, tests, version consistency
npm run tauri:build        # installers land in src-tauri/target/release/bundle/
```
Requirements: Node 22+, Rust (stable), and on Linux the WebKitGTK libraries (`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf`). On Windows, the "Desktop development with C++" build tools.

### Fully offline Windows installer
Windows 10 PCs without the Microsoft WebView2 runtime download it during installation, which fails without internet. For classrooms with no internet use:
```
npm run tauri:build:offline
```
This embeds the WebView2 runtime in the installer (about 130 MB larger). Windows 11 and up-to-date Windows 10 already include it.

## Releasing with GitHub Actions
1. Make sure `main` is green (the **CI** workflow runs type check, frontend tests, Rust tests, and a frontend build).
2. Change the version in **all three** places: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`. `npm run check:version` verifies they match.
3. Commit, then `git tag v1.0.0 && git push && git push --tags`.
4. The **Release** workflow builds all three platforms and creates a **draft** release with the installers attached. Open it on GitHub, test at least one installer per platform, then press *Publish*.

## Firewall (important for classrooms)
The teacher PC accepts connections from students on port 38123 (configurable). On Windows the NSIS installer adds an inbound allow rule for ProctorLAN on **Private and Domain** networks and removes it on uninstall. That needs administrator rights, so the installer offers "for all users" (administrator) or "just me". If the rule could not be added, Windows shows its normal "allow access" prompt the first time the teacher starts hosting; an administrator must approve it. If the classroom Wi-Fi is classed as **Public** in Windows, change it to Private. macOS asks "accept incoming connections?" on first run; Linux firewalls (ufw) need `sudo ufw allow 38123/tcp` if enabled. See `networking.md`.

## Signing (not done)
Installers are **unsigned**:
- **Windows** SmartScreen shows "Windows protected your PC" → *More info* → *Run anyway*.
- **macOS** Gatekeeper blocks the app → right-click the app → *Open* (or *System Settings → Privacy & Security → Open Anyway*).
- **Linux** has no equivalent warning.

Signing needs paid certificates (Windows code-signing certificate; Apple Developer account and notarisation). They can be added to the release workflow through repository secrets later; the build does not depend on them.

## Data and upgrades
App data (database, backups, exports) lives in the operating-system app-data folder for the identifier `app.proctorlan.desktop` and **survives** upgrades and uninstalls. Database migrations run automatically when a newer version starts. Install a newer version over the old one; to go back to an older version, restore a backup made by that older version (a newer database cannot be opened by an older app, and the app refuses it).

## What was verified, and what was not
Verified on Linux (Ubuntu-based build machine, headless X server):
- `npm run tauri:build -- --bundles deb` produces `ProctorLAN_0.1.0_amd64.deb` (about 4 MB) with the binary, desktop entry and icons.
- The release binary starts in a real WebKitGTK window, creates its database under the `app.proctorlan.desktop` data folder, applies migrations, and serves `GET /api/health` on the LAN port.
- Through the real window: the Welcome screen, first-run administrator setup (Argon2 in Rust), the Dashboard, and **Create backup now** (a verified `.db` appears in the backups list).

**Not verified** (never run by the author): the Windows `.exe`/`.msi` installers, the Windows firewall hook, the macOS `.dmg`, the Linux `.rpm`/`.AppImage`, and the GitHub Actions workflows. Run the release checklist on each platform before giving installers to students.
