# Checks when the installer hooks remove saved data, using test folders and a test registry key only.
$ErrorActionPreference = 'Stop'
$makensis = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'
if (-not (Test-Path $makensis)) { Write-Warning 'makensis not found (it is downloaded by the first installer build); hook test skipped.'; exit 0 }
$exe = Join-Path $env:TEMP 'akimov-hooktest.exe'
$dirs = @((Join-Path $env:APPDATA 'akimov-vpn-hooktest'), (Join-Path $env:LOCALAPPDATA 'vpn.akimov.hooktest'))
$key = 'HKCU:\Software\akimov-hooktest'
& $makensis /V2 "/DOUT=$exe" (Join-Path $PSScriptRoot 'uninstall-hook.nsi') | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'The hook test installer did not compile.' }
try {
  # name, arguments, app registered as installed, data expected to remain
  $cases = @(
    @('install with nothing installed', @(), $false, $false),
    @('install over an installed copy', @(), $true, $true),
    @('update with nothing registered', @('/UPDATE'), $false, $true),
    @('uninstall', @('/UNINSTALL'), $true, $false),
    @('uninstall during an update', @('/UNINSTALL', '/UPDATE'), $true, $true)
  )
  foreach ($case in $cases) {
    foreach ($dir in $dirs) { New-Item -ItemType Directory -Force (Join-Path $dir 'core') | Out-Null; 'x' | Set-Content (Join-Path $dir 'core\file.txt') }
    Remove-Item $key -Recurse -Force -ErrorAction SilentlyContinue
    if ($case[2]) { New-Item $key -Force | Out-Null; Set-ItemProperty $key UninstallString 'x' }
    if ($case[1].Count) { Start-Process $exe -ArgumentList $case[1] -Wait } else { Start-Process $exe -Wait }
    foreach ($dir in $dirs) { if ((Test-Path $dir) -ne $case[3]) { throw "$($case[0]): $dir should $(if ($case[3]) { 'remain' } else { 'be removed' })." } }
    $removed = (Get-ItemProperty $key -Name HelperRemoved -ErrorAction SilentlyContinue).HelperRemoved -eq 1
    $shouldRemove = ($case[1] -contains '/UNINSTALL') -and ($case[1] -notcontains '/UPDATE')
    if ($removed -ne $shouldRemove) { throw "$($case[0]): helper removal was $removed, expected $shouldRemove." }
  }
  'Installer hooks passed: saved data is removed on uninstall and clean install, kept on update.'
} finally {
  foreach ($dir in $dirs) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
  Remove-Item $key -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item $exe -Force -ErrorAction SilentlyContinue
}
