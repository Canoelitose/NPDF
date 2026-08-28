# Sets up a self-hosted GitHub Actions runner on Windows, including everything
# an NPDF build needs.
#
#   powershell -ExecutionPolicy Bypass -File scripts\runner\setup-windows.ps1 -Token <TOKEN>
#
# Run it in an Administrator PowerShell. The token comes from Settings,
# Actions, Runners, New self-hosted runner, and is valid for one hour.
#
# WARNING, read this before running it against a public repository:
# a self-hosted runner executes whatever a workflow tells it to, including a
# workflow from a stranger's pull request. Set Settings, Actions, General,
# Fork pull request workflows from outside collaborators, to
# "Require approval for all external contributors" first.

param(
  [Parameter(Mandatory = $true)][string]$Token,
  [string]$Name = "npdf-win",
  [string]$Path = "C:\a-npdf"
)

$ErrorActionPreference = "Stop"
$RepoUrl = "https://github.com/Canoelitose/NPDF"

if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()
    ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  throw "Bitte in einer PowerShell als Administrator starten."
}

Write-Host "==> Voraussetzungen"
# winget ships with Windows 11 and current Windows 10. Everything here is
# skipped when it is already installed.
$packages = @(
  @{ Id = "Rustlang.Rustup";                  Cmd = "rustup"  },
  @{ Id = "OpenJS.NodeJS.LTS";                Cmd = "node"    },
  @{ Id = "Git.Git";                          Cmd = "git"     },
  @{ Id = "Microsoft.EdgeWebView2Runtime";    Cmd = $null     },
  @{ Id = "Microsoft.VisualStudio.2022.BuildTools"; Cmd = $null }
)
foreach ($p in $packages) {
  if ($p.Cmd -and (Get-Command $p.Cmd -ErrorAction SilentlyContinue)) {
    Write-Host "    vorhanden: $($p.Id)"
    continue
  }
  Write-Host "    installiere: $($p.Id)"
  winget install --id $($p.Id) --silent --accept-source-agreements --accept-package-agreements | Out-Null
}

Write-Host "==> Lange Pfade und Defender"
# Rust and node_modules go deeper than the old 260 character limit, and
# Defender scanning every one of the tens of thousands of small files Rust
# writes costs a multiple of the build itself.
git config --system core.longpaths true 2>$null
New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" `
  -Name "LongPathsEnabled" -Value 1 -PropertyType DWord -Force | Out-Null
Add-MpPreference -ExclusionPath $Path -ErrorAction SilentlyContinue

Write-Host "==> Runner herunterladen, jeweils die aktuelle Ausgabe"
New-Item -ItemType Directory -Force -Path $Path | Out-Null
Set-Location $Path
$latest  = Invoke-RestMethod -Uri "https://api.github.com/repos/actions/runner/releases/latest"
$version = $latest.tag_name.TrimStart("v")
$archive = "actions-runner-win-x64-$version.zip"
Write-Host "    Version $version"
if (-not (Test-Path ".\config.cmd")) {
  Invoke-WebRequest -Uri "https://github.com/actions/runner/releases/download/v$version/$archive" -OutFile $archive
  Expand-Archive -Path $archive -DestinationPath $Path -Force
  Remove-Item $archive
}

Write-Host "==> Anmelden und als Dienst einrichten"
# On Windows the runner may configure itself with administrator rights, and
# --runasservice installs the Windows service in the same step.
.\config.cmd --url $RepoUrl --token $Token --name $Name `
  --labels self-hosted,Windows,X64,npdf,npdf-win `
  --work _work --unattended --replace --runasservice

Write-Host ""
Write-Host "Fertig. Der Runner sollte unter $RepoUrl/settings/actions/runners als Idle stehen."
Write-Host "Zum Einschalten die Repository-Variable WINDOWS_RUNNER auf $Name setzen."
Write-Host "Neue Konsole oeffnen, damit rustup und node im Pfad sind."
