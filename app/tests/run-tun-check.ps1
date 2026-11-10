$ErrorActionPreference = 'Stop'
$appRoot = Split-Path $PSScriptRoot -Parent
Push-Location $appRoot
try {
    & "$env:USERPROFILE\.cargo\bin\cargo.exe" test --manifest-path src-tauri/Cargo.toml windows_tun_smoke -- --ignored --nocapture *> (Join-Path $PSScriptRoot 'tun-check.log')
    $testExit = $LASTEXITCODE
    Set-Content -LiteralPath (Join-Path $PSScriptRoot 'tun-check.exit') -Value $testExit
} catch {
    $_ | Out-String | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'tun-check.log')
    Set-Content -LiteralPath (Join-Path $PSScriptRoot 'tun-check.exit') -Value 1
} finally { Pop-Location }
