param([string]$OutputDirectory = '', [switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    if (!$OutputDirectory) {
        $version = (Get-Content -LiteralPath 'src-tauri/tauri.conf.json' | ConvertFrom-Json).version
        $OutputDirectory = "dist/akimov-vpn-desktop-$version"
    }
    if (!$SkipBuild) {
        cargo build --release --locked --manifest-path src-tauri/Cargo.toml
        if ($LASTEXITCODE -ne 0) { throw 'Windows build failed.' }
    }
    $destination = Join-Path $PSScriptRoot $OutputDirectory
    New-Item -ItemType Directory -Force (Join-Path $destination 'core') | Out-Null
    Copy-Item -LiteralPath 'src-tauri/target/release/akimov-vpn.exe' -Destination $destination
    Copy-Item -LiteralPath 'src-tauri/core/mihomo-windows-amd64-compatible.exe' -Destination (Join-Path $destination 'core/mihomo.exe')
    Copy-Item -LiteralPath 'src-tauri/core/wintun.dll' -Destination (Join-Path $destination 'core/wintun.dll')
    Copy-Item -LiteralPath 'src-tauri/core/manifest.json' -Destination (Join-Path $destination 'core/manifest.json')
    Copy-Item -LiteralPath 'src-tauri/core/geoip.metadb','src-tauri/core/GeoSite.dat' -Destination (Join-Path $destination 'core')
    Copy-Item -LiteralPath 'core-patches' -Destination $destination -Recurse -Force
    Copy-Item -LiteralPath 'build-core.ps1' -Destination $destination
    Copy-Item -LiteralPath 'README.md' -Destination $destination
    if (Test-Path 'licenses') { Copy-Item -LiteralPath 'licenses' -Destination $destination -Recurse -Force }
    Copy-Item -LiteralPath 'ui/fonts/Martian-Mono-OFL.txt' -Destination (Join-Path $destination 'licenses/Martian-Mono-OFL.txt')
    Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $destination 'akimov-vpn.exe'),(Join-Path $destination 'core/mihomo.exe') | Format-Table
    Write-Host "Ready: $destination/akimov-vpn.exe"
} finally { Pop-Location }
