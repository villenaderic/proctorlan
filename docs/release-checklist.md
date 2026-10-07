# Release checklist

Run through this before publishing a release. Tick each item on a real machine; installers are unsigned and untested by the build alone.

## Before tagging
- [ ] `npm run release:check` passes (types, tests, versions match)
- [ ] `cargo test` passes in `src-tauri`
- [ ] `CHANGELOG.md` updated
- [ ] Version bumped in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`

## Each platform (install the draft release's installer on a clean machine)
- [ ] Installer runs; app starts; icon and name are correct
- [ ] First-run teacher setup, sign-out, sign-in
- [ ] Create an exam with all four question types; preview it
- [ ] Start a session; a **second computer** joins with the code and address (not just localhost)
- [ ] Student answers, loses Wi-Fi for ~10 s, reconnects: answers kept, teacher sees disconnect/reconnect
- [ ] Teacher pauses/resumes; time up auto-submits; results and CSV export look right
- [ ] Backup created; restore it and restart; data matches
- [ ] Uninstall removes the app (data folder remains) and, on Windows, the firewall rule
- [ ] Windows: works with Windows Firewall on, on a network classed Private
- [ ] Windows: installer on a PC without internet (use the offline installer if WebView2 is missing)

## Publishing
- [ ] Release notes mention: unsigned installers, first-run warnings, "signals not proof" for proctoring
- [ ] Draft release published; installers downloaded from it once and re-tested
