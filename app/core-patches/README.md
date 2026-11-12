# Bundled mihomo changes

This package uses mihomo v1.19.33 with `reality-client.patch`, `url-test-status.patch` and `url-test-keepalive.patch`, built with Go 1.26.10 and `with_gvisor`. The manifest records the source archive, patch and binary hashes. Rebuild with `./build-core.ps1`; Go and Git are required. Then build the Windows app with `./build.ps1`.

Modern REALITY servers require X25519MLKEM768. Upstream mihomo disables it by default, and its older Firefox fingerprint does not contain the hybrid share. The patch defaults to hybrid REALITY and uses the Chrome fingerprint when the requested fingerprint cannot supply that share. Explicit `support-x25519mlkem768: false` retains the legacy handshake. Certificate and REALITY authentication checks remain enabled.

The REALITY client version defaults to 26.3.27 for the modern handshake, replacing upstream's fixed 1.8.2. An optional `reality-opts.client-version` accepts three numbers in the range 0–255. This addresses servers configured with `minClientVer`; it does not claim support for every Xray transport or future feature.

References: [REALITY hybrid requirement](https://github.com/XTLS/REALITY/commit/8cdf7bf9c7f09cb9814bf08c3eb877f68b85fba8), [mihomo fingerprint issue](https://github.com/MetaCubeX/mihomo/issues/3193), [client-version issue](https://github.com/MetaCubeX/mihomo/issues/3263).

`tests/core-interop.cjs` transfers an HTTP response through a local Xray 26.10.31 REALITY/Vision server with minimum client version 26.3.27, using Chrome and Firefox configurations. It uses no user profile, system proxy or TUN adapter. Prepare the pinned Xray binary in `.tools/xray/xray.exe` and the local TLS fixture using `tests/prepare-interop.ps1`, then run `node tests/core-interop.cjs`. `--expect-failure` checks an unpatched core baseline.

The URL-test status patch rejects responses outside the requested expected HTTP status instead of returning a positive delay for a blocked page. The local Go test checks HTTP 204 success and HTTP 200/403/302 failures. The app tests each unique server through HTTPS, retries a second public 204 endpoint on failure, and keeps those measured results separate from background health histories. Each endpoint has a two-second budget; results stream to the picker as workers finish.

The app enables `unified-delay`: the first request warms the proxy/TLS connection, and the second request on that connection supplies the displayed time. The keep-alive patch rejects a closed connection or failed second request instead of silently reporting the cold request's handshake time. A local HTTPS test verifies two requests share one TLS connection, warm-up time is excluded, and `Connection: close` fails. Connections are closed after each probe; testing does not leave idle tunnels running.

The upstream source and these patches are GPL-3.0; see the bundled mihomo license.
