#!/bin/sh
# Builds Montana.app and installs it in /Applications.
set -eu
cd "$(dirname "$0")/.."

app=/Applications/Montana.app
new=target/Montana.app

cargo build --release --workspace
rm -rf "$new"
mkdir -p "$new/Contents/MacOS" "$new/Contents/Resources"
cp target/release/montana target/release/montana-bar "$new/Contents/MacOS/"
cp bar/ui/montana.icns bar/ui/fonts/OFL.txt "$new/Contents/Resources/"
cat > "$new/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>montana-bar</string>
  <key>CFBundleIconFile</key><string>montana</string>
  <key>CFBundleIdentifier</key><string>io.github.alrubearb-sys.montana</string>
  <key>CFBundleName</key><string>Montana</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$new/Contents/MacOS/montana"
codesign --force --sign - "$new"

pkill -x montana-bar || true
rm -rf "$app"
cp -R "$new" "$app"
echo "Installed $app"
