# Example: a home server behind Caddy

This folder is the maintainer's own setup: a ZimaOS box managed with [Arcane](https://github.com/ofkm/arcane), with Caddy terminating HTTPS. Use it as a template.

- `zimacube.compose.yaml`: the compose project. The container joins Caddy's network (`caddy_default`).
- `../scripts/deploy-zimacube.sh`: builds an amd64 image on a Mac, loads it on the server over SSH (`HOST=myserver`), and restarts the project. With published images you can instead set `image: ghcr.io/siganberg/vaultix:latest` and let Arcane/Watchtower pull updates.

## HTTPS with Caddy

```
drive.example.com {
	reverse_proxy vaultix:8080
}
```

Caddy sends `X-Forwarded-Proto: https`, so session cookies are marked `Secure` automatically.

## DNS tips

- **Cloudflare:** use a DNS-only (grey cloud) record. The orange-cloud proxy caps request bodies at 100 MB on the free plan, which breaks large uploads.
- **No hairpin NAT on your router?** Add a local DNS override (router or Pi-hole) that points the name to the server's LAN IP, so devices at home connect directly.
