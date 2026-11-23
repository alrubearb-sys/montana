# Template of the cask in alrubearb-sys/homebrew-peakcast. The release workflow
# fills in the version and the checksum and pushes it there.
cask "peakcast" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/alrubearb-sys/peakcast/releases/download/v#{version}/Peakcast-#{version}-macos-arm64.zip"
  name "Peakcast"
  desc "Mountain weather in the menu bar"
  homepage "https://github.com/alrubearb-sys/peakcast"

  depends_on arch: :arm64
  depends_on macos: :ventura

  app "Peakcast.app"
  binary "#{appdir}/Peakcast.app/Contents/MacOS/peakcast"

  # Started once here; from then on it starts itself at login.
  postflight_steps do
    run "/usr/bin/open", args: ["-g", "{{appdir}}/Peakcast.app"]
  end

  uninstall launchctl: "local.peakcast.bar",
            quit:      "io.github.alrubearb-sys.peakcast"

  zap trash: [
    "~/.cache/peakcast",
    "~/.config/peakcast",
    "~/Library/LaunchAgents/local.peakcast.bar.plist",
  ]
end
