akimov vpn 1.0.12 for Windows 10/11 x64.

- Subscription DNS resolvers, nameserver policies, fake-IP exclusions, hosts, IPv6 preference and transport settings survive profile loading and the protected helper rebuild. The app no longer substitutes its own DNS policy for a supplied one.
- Connected waits for ordinary local proxy traffic, not just the controller opening. Rule-provider loading can keep the controller available while mihomo is still rejecting application traffic.
- Whole device mode also checks its local DNS interception. A blocked DNS path reports an error instead of displaying a connected VPN with no usable DNS. Another active VPN can block this path; Browser only avoids creating a second TUN adapter.
- The whole-device helper is updated once with administrator consent so existing installations also retain subscription DNS settings. The replacement is verified before the old helper stops; later ordinary connections reuse the updated helper.

Choose setup.exe to install, or extract the portable ZIP with its core folder. In-app updates need the installed version. The installer is not code-signed for Windows SmartScreen; update packages are signed and verified by the app. Your other VPN is not disabled by this update.

Bundled core/driver/asset licenses, the routing database notice and the core patch are included. The matching upstream core source is included with the release.
