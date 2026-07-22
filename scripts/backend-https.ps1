# Run the Arna backend over HTTPS/WSS (for LAN calls without the Chrome flag).
# Needs the mkcert dev certs in infra/certs/ (see scripts/README.md).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$env:ARNA_TLS_CERT = "$root\infra\certs\dev-cert.pem"
$env:ARNA_TLS_KEY  = "$root\infra\certs\dev-key.pem"
$env:ARNA_DB       = "$root\services\arna-social.db"
$env:PORT          = "8787"
Set-Location "$root\services"
Write-Host "Backend -> https://<this-pc-ip>:8787" -ForegroundColor Green
go run .
