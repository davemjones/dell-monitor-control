#!/bin/sh
# Builds `target/Monitor Control.app`: compiles dellctl and the menu bar app,
# then bundles dellctl inside the app at Contents/Helpers/dellctl.
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
app="$repo/target/Monitor Control.app"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -n 1)

echo "Building dellctl ${version}…"
cargo build --release --manifest-path "$repo/Cargo.toml"

echo "Building Monitor Control…"
swift build -c release --package-path "$repo/macos-app"
bin=$(swift build -c release --package-path "$repo/macos-app" --show-bin-path)

# The icon is slow to draw, so only redraw it when its script changes.
icon="$repo/target/AppIcon.icns"
icon_script="$repo/macos-app/Icon/make-icon.swift"
if [ ! -f "$icon" ] || [ "$icon_script" -nt "$icon" ]; then
    echo "Drawing the app icon…"
    iconset="$repo/target/AppIcon.iconset"
    rm -rf "$iconset"
    swift "$icon_script" "$iconset"
    iconutil -c icns "$iconset" -o "$icon"
    rm -rf "$iconset"
fi

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Helpers" "$app/Contents/Resources"
cp "$bin/MonitorControl" "$app/Contents/MacOS/MonitorControl"
cp "$repo/target/release/dellctl" "$app/Contents/Helpers/dellctl"
cp "$icon" "$app/Contents/Resources/AppIcon.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>MonitorControl</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>com.davemjones.monitor-control</string>
    <key>CFBundleName</key>
    <string>Monitor Control</string>
    <key>CFBundleDisplayName</key>
    <string>Monitor Control</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$version</string>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

# Ad-hoc signatures so Apple Silicon will run it; not a distribution signature.
codesign --force --sign - "$app/Contents/Helpers/dellctl"
codesign --force --sign - "$app"

echo "Built $app"
echo "Install it by dragging it into /Applications, or try it in place with:"
echo "  open \"$app\""
