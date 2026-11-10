# akimov vpn for Windows

Windows 10/11 x64 client for the akimov cabinet, with whole-device TUN and per-user proxy modes.

Download the **setup.exe** or portable ZIP from [Releases](https://github.com/akimovvpn/akimov_vpn_windows/releases). The setup installs the app, core and Wintun together, adds shortcuts, and provides an uninstaller. In the portable ZIP, keep the `core` folder beside `akimov-vpn.exe`. WebView2 is required; setup downloads it if missing. Installers are currently unsigned.

Account features include plans, device management, wallet history, promo codes, referrals, and the cabinet's current legal-document links. The theme and service endpoints are fixed. Hardware identification is mandatory and derived from the firmware's system UUID; raw firmware identifiers are not transmitted. A subscription refresh registers the new identifier before connecting. An older installation-ID entry may remain in Devices after migration.

Closing hides the app to the tray by default. Disable **Settings → Application → Keep running in the tray** to quit on close. Tray **Quit** disconnects and exits. An idle app does not keep a core process running; disconnected latency tests use a temporary core without changing system routing.

## Build

Install Rust MSVC, Visual Studio C++ Build Tools, Node.js 24+, Go 1.26.10, and Git. Then:

```powershell
./app/build-core.ps1
./app/build.ps1
# Setup installer:
Set-Location app
npx --yes @tauri-apps/cli@2.12.4 build --ci --bundles nsis -- --locked
```

The GitHub Actions workflow builds releases for `v*` tags, runs the UI and Rust checks and local Xray interoperability test, and publishes setup, portable ZIP, source and checksums. The tag must match the version in `tauri.conf.json` and `Cargo.toml`.

## Validation and dependencies

See [app/README.md](app/README.md) for architecture, storage, payment limitations and optional network tests. Run the regular UI checks with `node app/tests/ui.test.cjs` and `node app/tests/ui-render.cjs`.

The separate mihomo executable is GPL-3.0. [Source patch and rebuild instructions](app/core-patches/README.md) and upstream source are available with releases. Wintun, fonts and flag assets retain their respective licenses in [app/licenses](app/licenses).

Local sessions, subscription files, diagnostic captures, private architecture notes, generated binaries and build output are excluded from Git. No panel or cabinet administration token is required.
