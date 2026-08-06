[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$InstallDir = Join-Path $env:LOCALAPPDATA 'Rustpture'
$InstalledExe = Join-Path $InstallDir 'Rustpture.exe'
$StartupShortcut = Join-Path ([Environment]::GetFolderPath('Startup')) 'Rustpture.lnk'
$MenuShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Rustpture.lnk'

if (Test-Path $InstalledExe) {
    & $InstalledExe --quit 2>$null
    Start-Sleep -Milliseconds 200
}

Remove-Item -Force -ErrorAction SilentlyContinue $StartupShortcut
Remove-Item -Force -ErrorAction SilentlyContinue $MenuShortcut
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $InstallDir

Write-Host 'Rustptureを削除しました。タスクバーに残ったアイコンは右クリックしてピン留めを外してください。'
