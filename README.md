# Vaultix

A small, fast, self-hosted cloud drive written in Rust: a single binary with an embedded web app, a native macOS menu-bar sync client, and WebDAV.

- **Fast**: async Rust (axum/tokio), streaming uploads and downloads, HTTP range requests (video seeking), BLAKE3 hashing, no PHP and no background-job queue.
- **Reliable**: the filesystem is the source of truth. Files are stored as plain files under `data/users/<name>/files/`, so there is no file-cache table that can drift. Uploads are written to a temporary file, fsynced, then atomically renamed into place.
- **Secure**: Argon2id passwords, TOTP 2FA with replay protection and one-time recovery codes, login lockout, per-device app passwords, `SameSite=Strict` HttpOnly cookies plus a CSRF header, CSP, and sandboxed inline file views.
- **Safe deletes**: everything deleted (web, sync, API) goes to Trash for 30 days. The web app has Undo.

## Server (Docker)

```bash
VAULTIX_ADMIN_PASSWORD='choose-a-strong-one' docker compose up -d --build
```

Open `http://<host>:8085`, sign in as `admin`, then go to **Settings → Two-factor authentication**.

| Env var | Default | |
|---|---|---|
| `VAULTIX_DATA` | `/data` | database + files |
| `VAULTIX_BIND` | `0.0.0.0:8080` | |
| `VAULTIX_ADMIN_USER` / `VAULTIX_ADMIN_PASSWORD` | – | creates the first admin on first start only |
| `VAULTIX_COOKIE_SECURE` | `auto` | `auto` = Secure when `X-Forwarded-Proto: https` |
| `VAULTIX_TRASH_DAYS` | `30` | |

Put it behind HTTPS (Caddy, Nginx Proxy Manager, Cloudflare Tunnel…) before exposing it to the internet.

### User admin

```bash
docker exec -it vaultix vaultix-server user add alice          # prompts for password
docker exec -it vaultix vaultix-server user add bob --admin
docker exec -it vaultix vaultix-server user passwd alice
docker exec -it vaultix vaultix-server user reset-2fa alice
docker exec -it vaultix vaultix-server user list
```

## Sharing

Right-click a file or folder in the web app:

- **Share link…**: anyone with the link can view the file or browse the folder (with previews), and download single files or everything as a zip. Options: password, expiry, view-only (downloads off).
- **Request files…** (folders): an upload-only page where anyone with the link can drop files into that folder without seeing what's in it. Options: password, expiry, max file size, a message for uploaders. Name clashes are auto-renamed, so nothing is overwritten, and the uploader's name (optional) is recorded.

All links are listed under **Shared** in the sidebar, with view/upload counts; edit or delete them there. Links use 128-bit random tokens (`/s/<token>`), password attempts are rate-limited, and links keep working when the shared item is moved or renamed.

## WebDAV

| URL | |
|---|---|
| `https://host/dav/` | standard |
| `https://host/remote.php/dav/files/<user>/` | Nextcloud-compatible |
| `https://host/remote.php/webdav/` | legacy Nextcloud/ownCloud |

Sign in with your username and an **app password** (Settings → Devices & app passwords). Your account password also works while 2FA is off. Finder: *Go → Connect to Server* → `https://host/dav/`.

WebDAV deletes are permanent (WebDAV has no trash semantics). Web and desktop-sync deletes go to Trash.

## macOS sync client

A SwiftUI menu-bar app (`mac/`) driving the Rust sync engine (`client/`) as a background helper.

```bash
./scripts/build-mac.sh          # → dist/Vaultix.app + dist/Vaultix-<version>.dmg (universal)
```

Open the DMG and drag Vaultix to Applications. The app is ad-hoc signed, so the first time you open it, right-click → **Open**. A sign-in window asks for your server, username and password; an app password also works and skips 2FA. You then pick the folders to sync.

- **Menu-bar popover**: status, live transfer progress, recent files (click one to reveal it in Finder), pause/resume, storage usage.
- **Settings**: start at login, notifications, account / sign out, change folders.
- **Notifications** for conflicts and sync problems.

How sync works: a three-way compare of local state, server state and the last-synced state.
- Changes on either side propagate.
- Edits on both sides keep both copies (`name (conflict <mac> <date>).ext`).
- Deletions go to the macOS Trash or the server Trash.
- If the folder suddenly looks empty (unmounted disk, wiped server), it refuses to mirror the deletion.
- Local changes sync about 2 s after they happen; remote changes are picked up within about 15 s.

The app talks to the helper (`vaultix-sync agent`) over JSON lines on stdin/stdout. The helper also runs on its own (Linux too):

```bash
vaultix-sync login https://host alice ~/Vaultix [server/folder]
vaultix-sync once     # one pass
vaultix-sync run      # continuous
```

Logs: `~/Library/Logs/Vaultix/sync.log`. UI previews with sample data: `mac/.build/debug/Vaultix --render-previews /tmp/previews`.

## Development

```bash
VAULTIX_DATA=./data VAULTIX_ADMIN_USER=admin VAULTIX_ADMIN_PASSWORD=devpass123 cargo run -p vaultix-server
```

The web UI (`web/`) is plain HTML/CSS/JS embedded into the binary at compile time; no build step.
