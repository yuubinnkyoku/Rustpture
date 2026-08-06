[CmdletBinding()]
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$SourceExe = Join-Path $Root 'dist\Rustpture.exe'
$InstallDir = Join-Path $env:LOCALAPPDATA 'Rustpture'
$InstalledExe = Join-Path $InstallDir 'Rustpture.exe'
$StartupDir = [Environment]::GetFolderPath('Startup')
$ProgramsDir = [Environment]::GetFolderPath('Programs')
$StartupShortcut = Join-Path $StartupDir 'Rustpture.lnk'
$MenuShortcut = Join-Path $ProgramsDir 'Rustpture.lnk'

if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'build-release.ps1')
}
if (-not (Test-Path $SourceExe)) {
    throw "実行ファイルがありません: $SourceExe"
}

if (Test-Path $InstalledExe) {
    & $InstalledExe --quit 2>$null
    Start-Sleep -Milliseconds 150
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -Force $SourceExe $InstalledExe
Copy-Item -Force (Join-Path $Root 'README.md') (Join-Path $InstallDir 'README.md')
Copy-Item -Force (Join-Path $Root 'LICENSE') (Join-Path $InstallDir 'LICENSE')

$Shell = New-Object -ComObject WScript.Shell

$Shortcut = $Shell.CreateShortcut($MenuShortcut)
$Shortcut.TargetPath = $InstalledExe
$Shortcut.Arguments = '--capture'
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.IconLocation = "$InstalledExe,0"
$Shortcut.Description = '画面範囲をピン留め'
$Shortcut.Save()

$Shortcut = $Shell.CreateShortcut($StartupShortcut)
$Shortcut.TargetPath = $InstalledExe
$Shortcut.Arguments = '--background'
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.IconLocation = "$InstalledExe,0"
$Shortcut.WindowStyle = 7
$Shortcut.Description = 'Rustptureを軽量常駐させる'
$Shortcut.Save()

Start-Process -FilePath $InstalledExe -ArgumentList '--background'
Write-Host "Installed: $InstalledExe"
Write-Host 'スタートメニューのRustptureを右クリックして、タスクバーへピン留めしてください。'
