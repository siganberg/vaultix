#!/usr/bin/env bash
# Build the amd64 image on this Mac, ship it to the ZimaCube and (re)start the Arcane project.
set -euo pipefail
cd "$(dirname "$0")/.."
HOST=${HOST:-zimacube}
PROJ=/DATA/PoolDrive/FastDrive/Services/arcane/data/projects/vaultix
DATA=/DATA/PoolDrive/FastDrive/Services/vaultix

docker buildx build --platform linux/amd64 -t vaultix:latest --load .
echo "Shipping image to $HOST…"
docker save vaultix:latest | gzip -1 | ssh "$HOST" 'gunzip | docker load'
ssh "$HOST" "mkdir -p $PROJ $DATA && test -w $DATA"
scp -q deploy/zimacube.compose.yaml "$HOST:$PROJ/compose.yaml"
ssh "$HOST" "cd $PROJ && docker compose -p vaultix up -d && docker image prune -f >/dev/null"
echo "Deployed: http://$HOST:8085"
