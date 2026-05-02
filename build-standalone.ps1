param(
    [switch]$Bundle,
    [switch]$DryRun,
    [switch]$Help
)

if ($Help) {
    @"
Build the native desktop app in direct mode (in-process stocker-core).

Usage:
  .\build-standalone.ps1                  # cargo release -> target\release\stocker-web.exe
  .\build-standalone.ps1 -Bundle          # dx bundle (requires dioxus-cli)
  .\build-standalone.ps1 -DryRun          # print commands only
  .\build-standalone.ps1 -Help            # this message
"@
    exit 0
}

$ErrorActionPreference = "Stop"

$repoRoot = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
$releaseExe = Join-Path $repoRoot "target\release\stocker-web.exe"

function Require-Command([string]$name, [string]$hint) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        throw "Missing command '$name'. $hint"
    }
}

$cargoArgs = @(
    "build", "-p", "stocker-web", "--release",
    "--no-default-features", "--features", "desktop"
)

if ($DryRun) {
    if ($Bundle) {
        Write-Host "[DryRun] Would run from frontend/: dx bundle --platform desktop --release --no-default-features"
    } else {
        Write-Host "[DryRun] Would run: cargo $($cargoArgs -join ' ')"
    }
    exit 0
}

if ($Bundle) {
    Require-Command "dx" "Install: cargo install dioxus-cli --locked"
    Write-Host "Bundling standalone desktop app (direct mode, release)..."
    Push-Location (Join-Path $repoRoot "frontend")
    try {
        dx bundle --platform desktop --release --no-default-features
    } finally {
        Pop-Location
    }
    Write-Host ""
    Write-Host "Done. See dx output above for bundle output directory (or use --out-dir)."
} else {
    Require-Command "cargo" "Install Rust and ensure cargo is on PATH."
    Set-Location $repoRoot
    Write-Host "Building standalone desktop binary (direct mode, release)..."
    & cargo @cargoArgs
    Write-Host ""
    Write-Host "Executable: $releaseExe"
}
