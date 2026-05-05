$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$generated = Join-Path $root "web\src-tauri\gen\android"
$target = Join-Path $root "app"

if (-not (Test-Path $generated)) {
  throw "[sync-generated-android] Generated Android project not found at $generated. Run `"npm run android:init`" in .android\web first."
}

Write-Host "[sync-generated-android] Mirroring $generated -> $target"
New-Item -ItemType Directory -Force -Path $target | Out-Null

robocopy $generated $target /MIR | Out-Null
$exitCode = $LASTEXITCODE
if ($exitCode -gt 7) {
  throw "[sync-generated-android] robocopy failed with exit code $exitCode"
}

Write-Host "[sync-generated-android] Done."
