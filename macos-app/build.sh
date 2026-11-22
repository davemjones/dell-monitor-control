#!/bin/bash
# Builds `target/Monitor Control.app` with Xcode: monitorctl (Rust), then the
# app and its MonitorControlCore framework, signed with the team set in
# Config/Signing.local.xcconfig. The Xcode project bundles monitorctl at
# Contents/Helpers/monitorctl and signs it with the app.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
mac="$repo/macos-app"
app="$repo/target/Monitor Control.app"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -n 1)

if [ ! -f "$mac/Config/Signing.local.xcconfig" ]; then
    echo "error: no signing team set" >&2
    echo "hint: copy macos-app/Config/Signing.local.xcconfig.example to Signing.local.xcconfig and set your Team ID" >&2
    exit 1
fi

# One version for monitorctl and the app, read by Config/MonitorControl.xcconfig.
printf 'MARKETING_VERSION = %s\nCURRENT_PROJECT_VERSION = %s\n' "$version" "$version" \
    > "$mac/Config/Version.xcconfig"

echo "Building monitorctl ${version}…"
cargo build --release --manifest-path "$repo/Cargo.toml"

# The icon is slow to draw, so only redraw it when its script changes.
icons="$mac/App/Assets.xcassets/AppIcon.appiconset"
icon_script="$mac/Icon/make-icon.swift"
if [ ! -f "$icons/icon_512x512@2x.png" ] || [ "$icon_script" -nt "$icons/icon_512x512@2x.png" ]; then
    echo "Drawing the app icon…"
    iconset="$repo/target/AppIcon.iconset"
    rm -rf "$iconset"
    swift "$icon_script" "$iconset"
    cp "$iconset"/*.png "$icons/"
    rm -rf "$iconset"
fi

echo "Building Monitor Control…"
xcodebuild -project "$mac/MonitorControl.xcodeproj" -scheme MonitorControl \
    -configuration Release -destination "generic/platform=macOS" -derivedDataPath "$repo/target/xcode" \
    -allowProvisioningUpdates -quiet build

rm -rf "$app"
ditto "$repo/target/xcode/Build/Products/Release/Monitor Control.app" "$app"
codesign --verify --deep --strict "$app"

echo "Built $app"
echo "Install it by dragging it into /Applications, or try it in place with:"
echo "  open \"$app\""
