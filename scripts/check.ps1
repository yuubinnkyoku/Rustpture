[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot

Push-Location $Root
try {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'cargo fmt --check failed' }

    cargo test --all-targets
    if ($LASTEXITCODE -ne 0) { throw 'cargo test failed' }

    cargo clippy --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'cargo clippy failed' }

    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw 'cargo build --release failed' }

    Write-Host 'All checks passed.'
}
finally {
    Pop-Location
}
