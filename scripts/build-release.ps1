[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$Dist = Join-Path $Root 'dist'

Push-Location $Root
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'cargo が見つかりません。RustのMSVCツールチェーンをインストールしてください。'
    }

    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release が失敗しました。終了コード: $LASTEXITCODE"
    }

    New-Item -ItemType Directory -Force -Path $Dist | Out-Null
    Copy-Item -Force (Join-Path $Root 'target\release\rustpture.exe') (Join-Path $Dist 'Rustpture.exe')
    Copy-Item -Force (Join-Path $Root 'README.md') (Join-Path $Dist 'README.md')
    Copy-Item -Force (Join-Path $Root 'LICENSE') (Join-Path $Dist 'LICENSE')

    Write-Host "Build complete: $(Join-Path $Dist 'Rustpture.exe')"
}
finally {
    Pop-Location
}
