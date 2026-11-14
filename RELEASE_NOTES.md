akimov vpn 1.0.11 for Windows 10/11 x64.

- A successful automatic-group ping now carries its actual working member and successful health URL into the real connection. Previously the temporary test core could find a working member while a newly connected core selected an unreachable one. Automatic failover remains enabled, and connecting still does not wait for a reachability test.
- Connected retests and rule reloads preserve the working automatic-group choice. Removed profile or provider members fall back to the current group instead of preventing connection.
- First use of Whole device mode installs a protected Windows helper with administrator consent. Subsequent ordinary connections and app launches reuse it without another prompt. Helper removal or incompatible helper maintenance can require administrator consent.
- The helper authenticates the owning Windows user, keeps its core controller private, rebuilds untrusted configurations in protected storage, and stops its core when the app disconnects, exits or crashes.
- Manual ping results remain in memory for the session. Process groups stay collapsed by default, with per-connection and per-process close actions.

Choose setup.exe to install, or extract the portable ZIP with its core folder. In-app updates need the installed version. The installer is not code-signed for Windows SmartScreen; update packages are signed and verified by the app. Your other VPN is not disabled by this update.

Bundled core/driver/asset licenses, the routing database notice and the core patch are included. The matching upstream core source is included with the release.
