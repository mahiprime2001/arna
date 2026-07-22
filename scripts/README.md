# Dev run scripts

Two ways to run the app for local development.

## Plain HTTP (localhost only)

```powershell
# terminal 1
cd services ; go run .
# terminal 2
cd client ; npm run dev
```

Open `http://localhost:4320`. Camera/mic (calls) work on `localhost` but NOT when
another device opens it via a LAN IP over http (browsers block mic/camera on
insecure origins).

## HTTPS (calls work between devices on the LAN, no Chrome flag)

One-time cert setup (host PC):

```powershell
go install filippo.io/mkcert@latest
& "$env:USERPROFILE\go\bin\mkcert.exe" -install
mkdir infra\certs -Force
& "$env:USERPROFILE\go\bin\mkcert.exe" -cert-file infra\certs\dev-cert.pem -key-file infra\certs\dev-key.pem <this-pc-lan-ip> localhost 127.0.0.1
```

Then run each in its own terminal:

```powershell
scripts\backend-https.ps1     # https/wss on :8787
scripts\client-https.ps1      # https on :4320
```

Open `https://<this-pc-lan-ip>:4320`.

### Trust the cert on OTHER devices (one-time each)

Copy the root CA from the host to the other device and install it:

- Host copy of the root CA: `infra\certs\rootCA.pem` (or `%LOCALAPPDATA%\mkcert\rootCA.pem`).
- On the other device: double-click `rootCA.pem` -> Install Certificate ->
  Current User -> "Place all certificates in the following store" ->
  **Trusted Root Certification Authorities**. (Or, in an admin prompt:
  `certutil -addstore -f Root rootCA.pem`.)

After that the other device opens `https://<host-ip>:4320` with a green lock and
calls work with no browser flag.

> Certs in `infra/certs/` are machine-specific and gitignored. Regenerate them
> per the one-time setup above on each host.
