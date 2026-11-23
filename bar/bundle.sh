#!/bin/sh
# Builds Peakcast.app and installs it in /Applications.
set -eu
cd "$(dirname "$0")/.."

app=/Applications/Peakcast.app
new=target/Peakcast.app

cargo build --release --workspace
rm -rf "$new"
mkdir -p "$new/Contents/MacOS" "$new/Contents/Resources"
cp target/release/peakcast target/release/peakcast-bar target/release/peakcast-map "$new/Contents/MacOS/"
cp bar/ui/peakcast.icns bar/ui/fonts/OFL.txt "$new/Contents/Resources/"
cat > "$new/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>peakcast-bar</string>
  <key>CFBundleIconFile</key><string>peakcast</string>
  <key>CFBundleIdentifier</key><string>io.github.alrubearb-sys.peakcast</string>
  <key>CFBundleName</key><string>Peakcast</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$new/Contents/MacOS/peakcast"
codesign --force --sign - "$new/Contents/MacOS/peakcast-map"
codesign --force --sign - "$new"

# The map too: while it runs, macOS takes Peakcast as open and does not start the panel.
pkill -x peakcast-bar || true
pkill -x peakcast-map || true
rm -rf "$app"
cp -R "$new" "$app"
echo "Installed $app"
