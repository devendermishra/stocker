param(
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$frontendDir = Join-Path $repoRoot "frontend"
$apiUrl = "http://127.0.0.1:8080"
$webUrl = "http://127.0.0.1:8081"

function Require-Command([string]$name, [string]$hint) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        throw "Missing command '$name'. $hint"
    }
}

Require-Command "cargo" "Install Rust and make sure cargo is on PATH."
Require-Command "dx" "Install Dioxus CLI: cargo install dioxus-cli --locked"

$dbPath = Join-Path $repoRoot "stocker.db"
$equityCsv = Join-Path $repoRoot "data\EQUITY_L.csv"
$envBlock = "`$env:STOCKER_DB_PATH = '$dbPath'; `$env:STOCKER_UNIVERSE_CSV = '$equityCsv'"
$apiCommand = "Set-Location '$repoRoot'; $envBlock; cargo run -p stocker-api"
$webCommand = "Set-Location '$frontendDir'; `$env:STOCKER_API_URL = '$apiUrl'; dx serve --port 8081"

Write-Host "Starting Stocker API + Web UI..."
Write-Host "API  : $apiUrl"
Write-Host "Web  : $webUrl"
Write-Host ""

if ($DryRun) {
    Write-Host "[DryRun] API command:"
    Write-Host "  $apiCommand"
    Write-Host "[DryRun] Web command:"
    Write-Host "  $webCommand"
    exit 0
}

Start-Process powershell -ArgumentList @("-NoExit", "-Command", $apiCommand) | Out-Null
Start-Process powershell -ArgumentList @("-NoExit", "-Command", $webCommand) | Out-Null

Write-Host "Launched two terminals:"
Write-Host "1) API server terminal"
Write-Host "2) Web UI terminal"
Write-Host ""
Write-Host "Open $webUrl in your browser."
