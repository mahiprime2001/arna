# Run the Arna client (vite) over HTTPS, LAN-visible.
# Needs the mkcert dev certs in infra/certs/ (see scripts/README.md).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$env:ARNA_TLS_CERT = "$root\infra\certs\dev-cert.pem"
$env:ARNA_TLS_KEY  = "$root\infra\certs\dev-key.pem"
Set-Location "$root\client"
Write-Host "Client -> https://<this-pc-ip>:4320" -ForegroundColor Green
npm run dev
