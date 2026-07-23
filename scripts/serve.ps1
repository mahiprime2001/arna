# Run all of Arna on ONE port: the built client + /api + /ws from the backend.
# Plain HTTP on purpose -- ngrok (or any tunnel) puts the HTTPS in front, so you
# need no certs and no Chrome flags. One port = one tunnel = everything works.
#
#   .\scripts\serve.ps1          # build the client, then serve on :8787
#   .\scripts\serve.ps1 -NoBuild # skip the rebuild (client/dist already fresh)
#
# Then, in a second terminal:  ngrok http 8787
param([switch]$NoBuild)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

if (-not $NoBuild) {
    Write-Host "Building the client..." -ForegroundColor Cyan
    Set-Location "$root\client"
    npm run build
}

$env:ARNA_WEB_DIR = "$root\client\dist"
$env:ARNA_DB      = "$root\services\arna-social.db"
$env:PORT         = "8787"

# No TLS here -- the tunnel terminates it. Clear any leftovers from the mkcert
# experiments so this always starts as plain HTTP.
$env:ARNA_TLS_CERT = ""
$env:ARNA_TLS_KEY  = ""

Set-Location "$root\services"
Write-Host ""
Write-Host "Arna (app + API + WS) -> http://localhost:8787" -ForegroundColor Green
Write-Host "Now run in another terminal:  ngrok http 8787" -ForegroundColor Yellow
Write-Host ""
go run .
