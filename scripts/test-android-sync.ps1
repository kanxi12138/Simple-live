$ErrorActionPreference = 'Stop'
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('simple-live-sync-' + [guid]::NewGuid().ToString('N'))
$sync = Join-Path $PSScriptRoot 'sync-generated-android.ps1'
New-Item -ItemType Directory -Path "$fixture/app/app/src/main", "$fixture/web/src-tauri/gen/android" -Force | Out-Null
try {
  git -C $fixture init --quiet
  Set-Content -LiteralPath "$fixture/app/app/src/main/saved.txt" -Value 'saved'
  Set-Content -LiteralPath "$fixture/app/local.properties" -Value 'fixture-only'
  New-Item -ItemType Directory -Path "$fixture/app/buildSrc/src/main/java/com/dtv/app/kotlin" -Force | Out-Null
  Set-Content -LiteralPath "$fixture/app/buildSrc/src/main/java/com/dtv/app/kotlin/BuildTask.kt" -Value 'saved-plugin'
  git -C $fixture add app
  Set-Content -LiteralPath "$fixture/web/src-tauri/gen/android/generated.txt" -Value 'generated'
  Set-Content -LiteralPath "$fixture/web/src-tauri/gen/android/local.properties" -Value 'keep-local'
  & $sync -RepositoryRoot $fixture
  if ((Get-Content "$fixture/web/src-tauri/gen/android/app/src/main/saved.txt") -ne 'saved') { throw 'Saved source missing' }
  if ((Get-Content "$fixture/web/src-tauri/gen/android/local.properties") -ne 'keep-local') { throw 'Local config overwritten' }
  if (-not (Test-Path "$fixture/web/src-tauri/gen/android/generated.txt")) { throw 'Generated file removed' }
  if ((Get-Content "$fixture/web/src-tauri/gen/android/buildSrc/src/main/java/www/sp/com/kotlin/BuildTask.kt") -ne 'saved-plugin') { throw 'Plugin path not migrated' }
  if (Test-Path "$fixture/web/src-tauri/gen/android/buildSrc/src/main/java/com/dtv/app/kotlin/BuildTask.kt") { throw 'Duplicate plugin created' }
  $rejected = $false
  try { & $sync -RepositoryRoot "$fixture/missing" } catch { $rejected = $true }
  if (-not $rejected) { throw 'Invalid root accepted' }
  Write-Host 'PASS: Android sync direction, generated files, local config and invalid root'
} finally {
  $resolved = [IO.Path]::GetFullPath($fixture)
  $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
  if ($resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and
      (Split-Path -Leaf $resolved).StartsWith('simple-live-sync-')) {
    Remove-Item -LiteralPath $resolved -Recurse -Force
  }
}
