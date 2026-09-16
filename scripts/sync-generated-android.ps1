param([string]$RepositoryRoot = (Split-Path -Parent $PSScriptRoot))
$ErrorActionPreference = "Stop"
$root = [System.IO.Path]::GetFullPath($RepositoryRoot)
$source = [System.IO.Path]::GetFullPath((Join-Path $root "app"))
$target = [System.IO.Path]::GetFullPath((Join-Path $root "web/src-tauri/gen/android"))
if (-not (Test-Path -LiteralPath $target -PathType Container)) {
  throw "Android project missing. Run android:init first."
}
# Refuse junctions/symlinks in either path; copying must stay inside this repository.
function Assert-LocalPath([string]$path) {
  $full = [System.IO.Path]::GetFullPath($path)
  if (-not $full.StartsWith($root.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw "Android sync path outside repository"
  }
  $cursor = $full
  while ($cursor -and $cursor -ne $root) {
    if ((Test-Path -LiteralPath $cursor) -and ((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
      throw "Android sync refuses reparse points"
    }
    $cursor = Split-Path -Parent $cursor
  }
}
Assert-LocalPath $source
Assert-LocalPath $target
$tracked = @(& git -C $root ls-files -- app)
if ($LASTEXITCODE -ne 0 -or $tracked.Count -eq 0) { throw "Cannot read tracked Android files" }
foreach ($entry in $tracked) {
  if ($entry -match '(^|/)(build|\.gradle|jniLibs|keystore)(/|$)|(^|/)(local|key)\.properties$|\.(apk|aab|jks|keystore|so)$') { continue }
  $from = [IO.Path]::GetFullPath((Join-Path $root $entry))
  # The saved Gradle plugin predates the package rename; overwrite the current generated copy.
  $relative = $entry.Substring(4) -replace '^buildSrc/src/main/java/com/dtv/app/kotlin/', 'buildSrc/src/main/java/www/sp/com/kotlin/'
  $to = [IO.Path]::GetFullPath((Join-Path $target $relative))
  Assert-LocalPath $from
  Assert-LocalPath $to
  if (-not $from.StartsWith($source + '\', [StringComparison]::OrdinalIgnoreCase) -or
      -not $to.StartsWith($target + '\', [StringComparison]::OrdinalIgnoreCase)) { throw "Invalid Android file path" }
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $to) | Out-Null
  Copy-Item -LiteralPath $from -Destination $to -Force
}
Write-Host "[android-sync] Saved Android sources applied to generated project."
