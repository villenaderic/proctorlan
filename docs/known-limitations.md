# Known limitations (1.0.0)

**NOT VERIFIED** (never run by the author):
- Windows `.msi`/NSIS installers, the firewall-rule hook, WebView2 offline bundle
- macOS `.dmg`, Gatekeeper behaviour
- RPM / AppImage bundles
- `.github/workflows/ci.yml` and `release.yml`
- Real multi-computer LAN, Wi-Fi client isolation, mDNS discovery across switches
- "Restart now" after staging a restore (use manual restart if it fails)

**By design**
- Not tamper-proof. A student with admin rights, a second device or a virtual machine can cheat; the app records signals (focus loss, disconnects), never penalises automatically, and cannot see other devices.
- No password recovery (offline); keep the admin password safe.
- Data is not encrypted at rest; OS access to the teacher's PC means access to the database.
- Installers are unsigned: Windows SmartScreen and macOS Gatekeeper will warn.
- Max 100 students per session.
- Backup/restore paths are typed in (no native file picker).
- Student side is plain HTTP/WS on the LAN, not TLS; use a trusted network.
