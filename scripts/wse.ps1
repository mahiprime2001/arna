# Start WSE Desktop: the local engine server + the Flutter UI.
# The server holds your workspaces (they persist while it runs); the UI is the
# client. Run:  powershell -ExecutionPolicy Bypass -File scripts\wse.ps1
$root = Split-Path $PSScriptRoot -Parent
$server = Join-Path $root 'engine\target\release\wse-server.exe'
$ui = Join-Path $root 'wse-ui\build\windows\x64\runner\Release\wse_ui.exe'

if (-not (Test-Path $server)) { Write-Host "Build the server: cargo build --release -p wse-server"; exit 1 }
if (-not (Test-Path $ui)) { Write-Host "Build the UI: (cd wse-ui) flutter build windows --release"; exit 1 }

if (-not (Get-Process wse-server -ErrorAction SilentlyContinue)) {
  Start-Process $server -WindowStyle Hidden
  Write-Host "started wse-server (holds your workspaces)"
}
Start-Sleep -Milliseconds 500
Start-Process $ui
Write-Host "started WSE UI"
