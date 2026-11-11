akimov vpn 1.0.2 for Windows 10/11 x64.

- An account that has not loaded yet is no longer shown as an empty account or as an error. Account, Plans and Devices show loading placeholders until the first successful answer, keep the last loaded data afterwards, and retry quietly in the background.
- A saved connection profile can connect while the account service is unreachable. Account data refreshes as soon as the VPN is up.
- Account requests fall back to the VPN route when the direct route is refused before reaching the account service. This fixes account screens in Browser only mode on networks where the API address is not reachable directly.
- Settings uses two columns that fill independently, so short groups no longer leave empty space.
- In-app updates: the app checks for a newer signed release shortly after start and from Settings → About. Installing verifies the release signature, disconnects the VPN, runs the installer and restarts the app.

Choose setup.exe to install, or extract the portable ZIP with its core folder. In-app updates need the installed version. The installer is not code-signed for Windows SmartScreen; update packages are signed and verified by the app. Your other VPN is not disabled by this update.

Bundled core/driver/asset licenses and the core patch are included. The matching upstream core source is included with the release.
