# build-windows.ps1 — Build and package Kairos for Windows
# PowerShell equivalent of build-windows.sh for GitLab hosted runners.

param(
    [ValidateSet("x86_64", "aarch64")]
    [string]$Arch = "x86_64",

    [string]$Features = "heatmap"
)

$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────────

function Step($msg)    { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Info($msg)    { Write-Host "info: $msg" -ForegroundColor Blue }
function Success($msg) { Write-Host "ok: $msg" -ForegroundColor Green }
function Err($msg)     { Write-Host "error: $msg" -ForegroundColor Red; exit 1 }

# ── Resolve paths ────────────────────────────────────────────────────────────

$RepoRoot = (Resolve-Path "$PSScriptRoot\..").Path
$CargoToml = Join-Path $RepoRoot "app\Cargo.toml"

if (-not (Test-Path $CargoToml)) {
    Err "Cannot find $CargoToml"
}

# ── Detect version ──────────────────────────────────────────────────────────

$VersionLine = Select-String -Path $CargoToml -Pattern '^version = "(.+)"' | Select-Object -First 1
if (-not $VersionLine) {
    Err "Could not detect version from $CargoToml"
}
$Version = $VersionLine.Matches[0].Groups[1].Value

# ── Resolve target ──────────────────────────────────────────────────────────

switch ($Arch) {
    "x86_64"  { $Target = "x86_64-pc-windows-msvc" }
    "aarch64" { $Target = "aarch64-pc-windows-msvc" }
}

$ExeName     = "kairos.exe"
$ArchiveName = "kairos-$Version-$Target.zip"

Step "Building Kairos v$Version for $Target"

# ── Prerequisites ────────────────────────────────────────────────────────────

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Err "'cargo' is required but not found in PATH"
}

# ── Verify assets ────────────────────────────────────────────────────────────

$AssetsDir = Join-Path $RepoRoot "assets"
$Missing = 0
foreach ($sub in @("fonts", "icons", "sounds")) {
    $subPath = Join-Path $AssetsDir $sub
    if (-not (Test-Path $subPath -PathType Container)) {
        Write-Host "error: Missing assets directory: assets\$sub" -ForegroundColor Red
        $Missing = 1
    }
}
if ($Missing -ne 0) { Err "Asset verification failed" }
Success "Assets verified"

# ── Build ────────────────────────────────────────────────────────────────────

Step "Compiling release binary"

rustup target add $Target 2>$null
if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    # Non-fatal: target may already be installed
}

$CargoArgs = @("build", "--release", "--target=$Target")
if ($Features) {
    $CargoArgs += "--features=$Features"
}

cargo @CargoArgs
if ($LASTEXITCODE -ne 0) { Err "cargo build failed" }

$Binary = Join-Path $RepoRoot "target\$Target\release\$ExeName"
if (-not (Test-Path $Binary)) {
    Err "Build succeeded but binary not found at $Binary"
}
Success "Binary built: $Binary"

# ── Package ──────────────────────────────────────────────────────────────────

Step "Creating archive: $ArchiveName"

$StagingDir = Join-Path ([System.IO.Path]::GetTempPath()) "kairos-staging-$([guid]::NewGuid().ToString('N').Substring(0,8))"
New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null

try {
    Copy-Item $Binary (Join-Path $StagingDir $ExeName)
    Copy-Item (Join-Path $RepoRoot "assets") (Join-Path $StagingDir "assets") -Recurse

    $OutputDir = Join-Path $RepoRoot "target\release"
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    $Archive = Join-Path $OutputDir $ArchiveName

    Compress-Archive -Path "$StagingDir\*" -DestinationPath $Archive -Force
    Success "Archive created"
} finally {
    Remove-Item $StagingDir -Recurse -Force -ErrorAction SilentlyContinue
}

# ── Checksum ─────────────────────────────────────────────────────────────────

$Hash = (Get-FileHash -Path $Archive -Algorithm SHA256).Hash.ToLower()
$ChecksumFile = "$Archive.sha256"
"$Hash  $(Split-Path $Archive -Leaf)" | Out-File -FilePath $ChecksumFile -Encoding ascii -NoNewline
Success "Checksum written: $(Split-Path $ChecksumFile -Leaf)"

# ── Summary ──────────────────────────────────────────────────────────────────

$Size = (Get-Item $Archive).Length
Write-Host ""
Write-Host "-- Artifact ----------------------------------" -ForegroundColor White
Write-Host "  File:   $ArchiveName"
Write-Host "  Size:   $Size bytes"
Write-Host "  SHA256: $Hash"
Write-Host "----------------------------------------------" -ForegroundColor White
