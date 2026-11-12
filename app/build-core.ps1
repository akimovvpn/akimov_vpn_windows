param([string]$Go = '', [string]$OutputDirectory = '')
$ErrorActionPreference = 'Stop'
$version = '1.19.33'
$sourceHash = 'BB705AA873FC0E9DB352FDC69B992C93FAD5B310756E0E5F81194A0E0C108E21'
$workspace = Split-Path $PSScriptRoot -Parent
$cache = Join-Path $workspace '.tools'
New-Item -ItemType Directory -Force $cache | Out-Null
if (!$Go) {
    $portable = Join-Path $cache 'go/bin/go.exe'
    if (Test-Path -LiteralPath $portable) { $Go = $portable } else { $Go = (Get-Command go -ErrorAction Stop).Source }
}
if (!$OutputDirectory) { $OutputDirectory = Join-Path $PSScriptRoot 'src-tauri/core' }
$archive = Join-Path $cache "mihomo-v$version.zip"
if (!(Test-Path -LiteralPath $archive)) {
    Invoke-WebRequest "https://codeload.github.com/MetaCubeX/mihomo/zip/refs/tags/v$version" -OutFile $archive
}
if ((Get-FileHash -LiteralPath $archive).Hash -ne $sourceHash) { throw 'Core source checksum mismatch.' }
$build = Join-Path $cache 'patched-core-build'
$source = Join-Path $build "mihomo-$version"
if (!(Test-Path -LiteralPath $source)) { Expand-Archive -LiteralPath $archive -DestinationPath $build }
if (!(Test-Path -LiteralPath (Join-Path $source '.git'))) {
    git init --quiet $source
    if ($LASTEXITCODE -ne 0) { throw 'Could not isolate the upstream source checkout.' }
}
$patch = Join-Path $PSScriptRoot 'core-patches/reality-client.patch'
$urlPatch = Join-Path $PSScriptRoot 'core-patches/url-test-status.patch'
$keepAlivePatch = Join-Path $PSScriptRoot 'core-patches/url-test-keepalive.patch'
Push-Location $source
try {
    foreach ($corePatch in @($patch, $urlPatch, $keepAlivePatch)) {
        git apply --check $corePatch 2>$null
        if ($LASTEXITCODE -eq 0) {
            git apply $corePatch
            if ($LASTEXITCODE -ne 0) { throw 'Core patch failed.' }
        } else {
            git apply --reverse --check $corePatch
            if ($LASTEXITCODE -ne 0) { throw 'Core source does not match the patch.' }
        }
    }
    & $Go test -buildvcs=false ./component/tls ./adapter/outbound ./adapter -run 'Test(RealityClientVersions|ModernRealityHelloAndLegacyOptOut|URLTestRequiresExpectedStatus|URLTestUnifiedKeepAlive)' -count=1
    if ($LASTEXITCODE -ne 0) { throw 'Core tests failed.' }
    New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
    $binary = Join-Path $OutputDirectory 'mihomo-windows-amd64-compatible.exe'
    & $Go build -buildvcs=false -tags with_gvisor -trimpath -ldflags '-s -w -X github.com/metacubex/mihomo/constant.Version=v1.19.33-akimov.3' -o $binary .
    if ($LASTEXITCODE -ne 0) { throw 'Core build failed.' }
    # Routing databases for the subscription's GEOIP/GEOSITE rules, shipped so the core never downloads them when connecting.
    $geoip = Join-Path $OutputDirectory 'geoip.metadb'
    $geosite = Join-Path $OutputDirectory 'GeoSite.dat'
    Invoke-WebRequest 'https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geoip.metadb' -OutFile $geoip
    Invoke-WebRequest 'https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geosite.dat' -OutFile $geosite
    if ((Get-Item -LiteralPath $geoip).Length -lt 1MB -or (Get-Item -LiteralPath $geosite).Length -lt 1MB) { throw 'A routing database download is incomplete.' }
    [ordered]@{
        version = 'v1.19.33-akimov.3'
        upstream = "https://github.com/MetaCubeX/mihomo/tree/v$version"
        source_archive_sha256 = $sourceHash.ToLowerInvariant()
        patch_sha256 = (Get-FileHash -LiteralPath $patch).Hash.ToLowerInvariant()
        url_test_patch_sha256 = (Get-FileHash -LiteralPath $urlPatch).Hash.ToLowerInvariant()
        url_test_keepalive_patch_sha256 = (Get-FileHash -LiteralPath $keepAlivePatch).Hash.ToLowerInvariant()
        binary_sha256 = (Get-FileHash -LiteralPath $binary).Hash.ToLowerInvariant()
        geoip_sha256 = (Get-FileHash -LiteralPath $geoip).Hash.ToLowerInvariant()
        geosite_sha256 = (Get-FileHash -LiteralPath $geosite).Hash.ToLowerInvariant()
        toolchain = (& $Go version)
    } | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $OutputDirectory 'manifest.json')
} finally { Pop-Location }
