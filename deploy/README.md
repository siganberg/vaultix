# ZimaCube deployment

- Arcane project: `/DATA/PoolDrive/FastDrive/Services/arcane/data/projects/vaultix/compose.yaml` (= `zimacube.compose.yaml`)
- Data: `/DATA/PoolDrive/FastDrive/Services/vaultix` (owned by uid 1000)
- LAN: http://zimacube:8085 · Public: https://vaultix.marasigan.net
- Update: `./scripts/deploy-zimacube.sh` (builds amd64 on the Mac, ships the image, restarts)

## HTTPS
Caddy (`/DATA/PoolDrive/FastDrive/Services/caddy/Caddyfile`), the container joins `caddy_default`:

```
vaultix.marasigan.net {
	reverse_proxy vaultix:8080
}
```

DNS:
- Cloudflare: `A vaultix → <public IP>`, **DNS only (grey cloud)**. The orange-cloud proxy caps request bodies at 100 MB on the free plan, which breaks large uploads.
- Router (10.0.0.1) local override: `vaultix.marasigan.net → 10.0.0.138`, like nextcloud/photos.

Caddy sends `X-Forwarded-Proto: https`, so session cookies are marked `Secure` automatically.
