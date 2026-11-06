#!/usr/bin/env bash
# Builds Vaultix.app (menu-bar sync client) and a drag-to-install DMG.
#   ./scripts/build-mac.sh            # universal (arm64 + x86_64) if both targets are installed
#   ./scripts/build-mac.sh --native   # current architecture only (faster)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' client/Cargo.toml | cut -d'"' -f2)
OUT=dist
APP="$OUT/Vaultix.app"
rm -rf "$APP" "$OUT/Vaultix-$VERSION.dmg"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

if [[ "${1:-}" != "--native" ]] && command -v rustup >/dev/null; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
  cargo build --release -p vaultix-sync --target aarch64-apple-darwin
  cargo build --release -p vaultix-sync --target x86_64-apple-darwin
  lipo -create -output "$APP/Contents/MacOS/vaultix-sync" \
    target/aarch64-apple-darwin/release/vaultix-sync target/x86_64-apple-darwin/release/vaultix-sync
else
  cargo build --release -p vaultix-sync
  cp target/release/vaultix-sync "$APP/Contents/MacOS/vaultix-sync"
fi

# App icon from web/icon.svg (Quick Look renders the SVG; iconutil packs the sizes).
ICONSET=$(mktemp -d)/Vaultix.iconset
mkdir -p "$ICONSET"
if qlmanage -t -s 1024 -o "$(dirname "$ICONSET")" web/icon.svg >/dev/null 2>&1; then
  SRC="$(dirname "$ICONSET")/icon.svg.png"
  for s in 16 32 128 256 512; do
    sips -z $s $s "$SRC" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    sips -z $((s*2)) $((s*2)) "$SRC" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Vaultix.icns" || true
fi

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Vaultix</string>
  <key>CFBundleDisplayName</key><string>Vaultix</string>
  <key>CFBundleIdentifier</key><string>app.vaultix.sync</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>vaultix-sync</string>
  <key>CFBundleIconFile</key><string>Vaultix</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAppleEventsUsageDescription</key><string>Vaultix shows setup dialogs.</string>
  <key>NSLocalNetworkUsageDescription</key><string>Vaultix connects to your Vaultix server on your home network to sync your files.</string>
</dict></plist>
EOF

# Ad-hoc signature (no Apple developer account needed). Use your Developer ID here to notarize.
codesign --force --deep --sign "${CODESIGN_ID:--}" "$APP"

# DMG with an Applications shortcut.
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Vaultix" -srcfolder "$STAGE" -ov -format UDZO "$OUT/Vaultix-$VERSION.dmg" >/dev/null
rm -rf "$STAGE"

echo "Built $APP and $OUT/Vaultix-$VERSION.dmg"
