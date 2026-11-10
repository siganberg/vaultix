# Vaultix

A small, fast, self-hosted cloud drive. Run the server as a **desktop app** (no Docker needed) or as a Docker container, and sync with desktop apps for macOS, Windows and Linux. It also speaks WebDAV for everything else.

- **Fast**: async Rust (axum/tokio), streaming uploads and downloads, HTTP range requests (video seeking), BLAKE3 hashing. No PHP and no background-job queue.
- **Reliable**: your files are stored as plain files on disk (`data/users/<name>/files/`), so nothing can drift out of sync with a database. Uploads are written to a temporary file, flushed, then atomically renamed into place.
- **Secure**: Argon2id passwords, two-factor authentication with recovery codes, login lockout, per-device app passwords, strict cookies plus a CSRF header, and a CSP.
- **Safe deletes**: everything you delete (in the web app, a sync client or the API) goes to Trash for 30 days, and the web app has Undo.
- **Sharing**: share links (with password, expiry and view-only options) and upload-only "file request" links.
- **Previews**: photos (zoom), video, audio, PDF, Markdown, CSV, code, **STL/OBJ in 3D** and **G-code toolpaths**.

---

## 1. Run the server

Pick one. Both give you the same Vaultix; you can move between them later because the data folder has the same layout.

| | Best for |
|---|---|
| **A. Vaultix Server app** | a Windows PC, Mac or Linux desktop that stays on. No Docker, no terminal. |
| **B. Docker** | a NAS, home server or VPS |

### A. Vaultix Server app (no Docker)

1. Download **Vaultix Server** for your computer from the **[latest release](https://github.com/siganberg/vaultix/releases/latest)**: `VaultixServer-<version>.dmg` (macOS), `VaultixServer_<version>_x64-setup.exe` (Windows), or `VaultixServer_<version>_amd64.deb` / `.AppImage` (Linux). The first-launch warnings are the same as for the sync apps; see [Install the apps](#2-install-the-apps).
2. Open it. Choose where files should be kept (default: a `Vaultix Server` folder in your home folder) and click **Start Server**.
3. Click **Open Vaultix** and create your admin account in the browser.
4. The window shows the address for your other devices, for example `http://192.168.1.24:8085`. Type it into a browser, or into the Vaultix app on your other computers.

Good to know:

- Vaultix Server lives in the menu bar or system tray, and starts by itself when you sign in to the computer (you can turn that off). Closing the window keeps it running; **Quit** stops the server.
- Keep the computer **on and awake**. While it sleeps, other devices can't reach your files.
- Windows asks once whether to allow Vaultix Server through the firewall. Choose **Allow** for private networks.
- On the server computer you don't need the sync app: your files are already there as normal files (**Open** next to *Files location*). Changes you make in that folder reach your other devices within seconds.
- This works on your home network out of the box. To use it away from home, see [Making it reachable from anywhere](#making-it-reachable-from-anywhere-https), or use a private network such as [Tailscale](https://tailscale.com) and connect to the computer's Tailscale address.

### B. Docker

You need Docker on any Linux box, NAS, Mac or Windows PC. Save this as `docker-compose.yml`:

```yaml
services:
  vaultix:
    image: ghcr.io/siganberg/vaultix:latest
    container_name: vaultix
    restart: unless-stopped
    ports:
      - "8085:8080"
    environment:
      TZ: America/Los_Angeles          # your time zone
    volumes:
      - ./data:/data                   # database + everyone's files
```

```bash
docker compose up -d
```

Open **http://&lt;server-ip&gt;:8085**. The first time, Vaultix asks you to **create the admin account** in the browser. No terminal commands are needed.

### After the first start (both options)

1. **Settings → Two-factor authentication** to protect your account.
2. **Settings → Users → Add user** to give family or teammates their own private drive.
3. Install the apps (below).

> Create the admin account right after the first start. Until it exists, anyone who can reach the server could create it.

Images are published for `linux/amd64` and `linux/arm64` (Raspberry Pi 4/5 and ARM NASes work) on GitHub Container Registry, `ghcr.io/siganberg/vaultix`, and on Docker Hub. Tags:

| Tag | What you get |
|---|---|
| `latest` | the newest release |
| `0.3` | the newest 0.3.x release |
| `0.3.0` | exactly that version |
| `edge` | the latest `main` build |

### Making it reachable from anywhere (HTTPS)

Put Vaultix behind a reverse proxy with HTTPS before you expose it to the internet. (With the Vaultix Server app, run a reverse proxy such as Caddy on the same computer and point it at `localhost:8085`.) With Docker and **Caddy**, add this to your compose file next to Vaultix:

```yaml
services:
  vaultix:
    image: ghcr.io/siganberg/vaultix:latest
    restart: unless-stopped
    environment:
      TZ: UTC
    volumes:
      - ./data:/data

  caddy:
    image: caddy:2
    restart: unless-stopped
    ports: ["80:80", "443:443"]
    command: caddy reverse-proxy --from drive.example.com --to vaultix:8080
    volumes:
      - ./caddy:/data
```

Point `drive.example.com` at your public IP, and forward ports 80 and 443 on your router. Caddy gets a certificate automatically. **Nginx Proxy Manager**, **Traefik** and **Cloudflare Tunnel** work too: proxy to port 8080. If you use Cloudflare, use a *DNS-only* (grey-cloud) record, because the orange-cloud proxy limits uploads to 100 MB on the free plan.

### Settings

| Env var | Default | |
|---|---|---|
| `TZ` | `UTC` | time zone for logs |
| `VAULTIX_TRASH_DAYS` | `30` | how long deleted files stay in Trash |
| `VAULTIX_COOKIE_SECURE` | `auto` | `auto` = Secure cookies when the proxy says HTTPS (`X-Forwarded-Proto`) |
| `VAULTIX_ADMIN_USER` / `VAULTIX_ADMIN_PASSWORD` | – | optional: create the admin on first start instead of in the browser |
| `VAULTIX_DATA`, `VAULTIX_BIND` | `/data`, `0.0.0.0:8080` | rarely needed |
| `VAULTIX_WATCH` | `0` | `1` = watch the data folder, so files changed directly on disk reach sync clients within seconds (always on in the Vaultix Server app) |

**Backups:** back up the data folder (`./data` with Docker, your *Files location* with the app). Files are plain files, and the database is `data/vaultix.db`.

**Updating:** run `docker compose pull && docker compose up -d`.

---

## 2. Install the apps

Download the **Vaultix** sync app for each computer from the **[latest release](https://github.com/siganberg/vaultix/releases/latest)**. When you open it, a sign-in window asks for:

- **Server**: the address you open in the browser, for example `https://drive.example.com`.
- **Username** and **password**: if you turned on 2FA, it then asks for the 6-digit code. An *app password* (Settings → Devices & app passwords) also works and skips 2FA.
- **Which folder to sync**: by default a `Vaultix` folder in your home folder, synced with your whole drive. You can also choose a single server folder such as `/Projects`.

After that, Vaultix lives in your menu bar or system tray. It shows what's syncing and your recent files, and lets you pause, open your folder or open the web app.

### macOS

1. Download `Vaultix-<version>.dmg`, open it and drag **Vaultix** to **Applications**.
2. The first time, **right-click Vaultix → Open**, then **Open** again. The app isn't notarized by Apple yet, so a normal double-click is blocked once.
3. If macOS asks whether Vaultix may find devices on your **local network**, click **Allow**. This is needed when your server is on your home network.

Requires macOS 13 or later (Apple Silicon and Intel).

### Windows

1. Download `Vaultix_<version>_x64-setup.exe` and run it. It installs for your user only, so no admin rights are needed.
2. Windows SmartScreen may say *"Windows protected your PC"* because the app isn't code-signed yet. Click **More info → Run anyway**.
3. Vaultix appears in the notification area of the taskbar. Click the icon to open it; right-click for the menu. Tip: drag the icon out of the "^" overflow area so it's always visible.

Requires Windows 10 or 11 (64-bit). The installer adds Microsoft's WebView2 runtime if it's missing.

### Linux

- **Ubuntu, Debian, Mint, Pop!_OS:** download `vaultix_<version>_amd64.deb`, then
  ```bash
  sudo apt install ./vaultix_*_amd64.deb
  ```
- **Any distro:** download `Vaultix_<version>_amd64.AppImage`, make it executable (`chmod +x`, or right-click → Properties → *Allow executing*) and run it.

Start **Vaultix** from your app menu. It adds a tray icon; on **GNOME**, install the *AppIndicator and KStatusNotifierItem Support* extension (preinstalled on Ubuntu) to see it. Use **Open Vaultix** in the tray menu for the status window.

**Headless servers:** `vaultix-sync-<version>-linux-x86_64.tar.gz` contains a command-line sync client:

```bash
vaultix-sync login https://drive.example.com alice ~/Vaultix [/server/folder]
vaultix-sync run      # keep syncing (run it from systemd or tmux)
vaultix-sync once     # a single pass
```

### How sync works

Vaultix does a three-way compare: the files on this computer, the files on the server, and the state at the last sync.

- Changes on either side are copied to the other: local changes within about 2 seconds, server changes within about 15 seconds.
- A file edited in both places keeps **both** copies (`name (conflict <computer> <date>).ext`), so nothing is lost.
- Deleted files go to your computer's Trash (or Recycle Bin), or the server's Trash.
- If the folder suddenly looks empty (an unplugged drive, a wiped server), Vaultix refuses to mirror the deletion.

---

## Sharing

Right-click a file or folder in the web app:

- **Share link…**: anyone with the link can view the file or browse the folder (with previews) and download single files or everything as a zip. Options: password, expiry, view-only.
- **Request files…** (folders only): an upload-only page where anyone with the link can drop files into that folder without seeing what's in it. Options: password, expiry, maximum file size, and a message for uploaders. Name clashes are renamed automatically, so nothing is overwritten.

All links are listed under **Shared** in the sidebar.

## WebDAV

| URL | |
|---|---|
| `https://host/dav/` | standard |
| `https://host/remote.php/dav/files/<user>/` | Nextcloud-compatible |
| `https://host/remote.php/webdav/` | legacy Nextcloud/ownCloud |

Sign in with your username and an **app password** (Settings → Devices & app passwords). Your normal password also works while 2FA is off. On a Mac: Finder → *Go → Connect to Server* → `https://host/dav/`. WebDAV deletes are permanent; web and desktop deletes go to Trash.

## Command-line admin (optional)

Everything is available in the web app, but these commands also work:

```bash
docker exec -it vaultix vaultix-server user add alice [--admin]   # prompts for a password
docker exec -it vaultix vaultix-server user passwd alice
docker exec -it vaultix vaultix-server user reset-2fa alice
docker exec -it vaultix vaultix-server user list
```

---

## Development

```bash
# Server + web app (the web UI is plain HTML/CSS/JS embedded at compile time)
VAULTIX_DATA=./data cargo run -p vaultix-server            # http://localhost:8080

# macOS app (SwiftUI + the Rust sync engine)
./scripts/build-mac.sh                                     # dist/Vaultix.app + .dmg

# Windows/Linux app (Tauri + the same sync engine, in-process)
cd desktop && cargo tauri dev                              # needs: cargo install tauri-cli
```

| Folder | |
|---|---|
| `server/` | axum server, REST API, WebDAV, sharing |
| `web/` | web app (no build step) |
| `client/` | sync engine (library + `vaultix-sync` CLI/agent) |
| `mac/` | macOS menu-bar app (SwiftUI) |
| `desktop/` | Windows/Linux tray app (Tauri; HTML UI in `desktop/ui`) |
| `server-app/` | Vaultix Server desktop app (Tauri; runs `server/` in-process) |

### Releasing

```bash
scripts/release.sh 0.4.0 notes.md      # bumps versions, tags v0.4.0 with notes.md as the message, pushes
```

The tag triggers `.github/workflows/release.yml`. It builds the multi-arch Docker image (pushed to GHCR, and to Docker Hub when `DOCKERHUB_USERNAME`/`DOCKERHUB_TOKEN` secrets are set), plus the sync app and the Vaultix Server app for macOS (DMG), Windows (installer) and Linux (`.deb`/`.AppImage`), and the headless sync CLI. It then publishes a GitHub Release with the notes. To edit the notes later, run `scripts/release.sh --notes v0.4.0 notes.md`.

## License

[GNU GPL v3.0](LICENSE). You may use, modify and share Vaultix; if you distribute a modified version, it must also be released under the GPL v3 with its source code.
