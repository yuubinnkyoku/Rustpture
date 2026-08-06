[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$Dist = Join-Path $Root 'dist'
$Stage = Join-Path $Dist 'Rustpture-0.1.0-windows-x64'
$Archive = Join-Path $Dist 'Rustpture-0.1.0-windows-x64.zip'

& (Join-Path $PSScriptRoot 'build-release.ps1')

Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Stage
Remove-Item -Force -ErrorAction SilentlyContinue $Archive
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item -Force (Join-Path $Dist 'Rustpture.exe') $Stage
Copy-Item -Force (Join-Path $Root 'README.md') $Stage
Copy-Item -Force (Join-Path $Root 'LICENSE') $Stage
Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $Archive -CompressionLevel Optimal
Remove-Item -Recurse -Force $Stage

Write-Host "Release archive: $Archive"
