$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$source = Join-Path $root "core"
$target = Join-Path $root "web\src-tauri"

Write-Host "[sync-core] Mirroring $source -> $target"
New-Item -ItemType Directory -Force -Path $target | Out-Null

robocopy $source $target /MIR /XD target gen | Out-Null
$exitCode = $LASTEXITCODE
if ($exitCode -gt 7) {
  throw "[sync-core] robocopy failed with exit code $exitCode"
}

Write-Host "[sync-core] Done."
