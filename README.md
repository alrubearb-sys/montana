# Peakcast

Mountain weather for the places you go: summit and base, wind at the height of the summit, snow
line and freezing level, in your menu bar.

Peakcast has three parts, written in Rust:

- `peakcast`, the engine: a command-line tool that finds places, keeps your areas and prints their
  forecast, also as JSON.
- `peakcast-bar`, a menu bar panel for the Mac with the forecast and an hourly chart. A plugin for
  the [Omarchy](https://omarchy.org/) bar is on the way.
- `peakcast-map`, a topographic map to place the summit and the base of an area.

> Status: early days. This is a learning project, so things will change and break.

## Install

On a Mac with Apple Silicon:

```
brew tap alrubearb-sys/peakcast
brew trust alrubearb-sys/peakcast
brew install --cask peakcast
```

The panel starts at once and, from then on, at login; `brew upgrade --cask peakcast` updates it.

On Linux, the engine and the map are in each [release](https://github.com/alrubearb-sys/peakcast/releases)
for x86_64 and aarch64. An AUR package and the Omarchy plugin are coming.

## Using the engine

```
peakcast search Säntis                 # find a place, anywhere in the world
peakcast add Säntis                    # save it as an area
peakcast areas                         # list your areas
peakcast Säntis                        # its forecast
peakcast panel Säntis                  # what the panels show, as JSON
peakcast move Säntis --summit 47.2494,9.3432 --base 47.2583,9.3622
peakcast remove Säntis
```

## Data sources

- [Open-Meteo](https://open-meteo.com/): forecast and elevation (CC BY 4.0).
- [Nominatim](https://nominatim.openstreetmap.org/): place search, © OpenStreetMap contributors (ODbL).
- [swisstopo](https://www.swisstopo.admin.ch/): elevations in Switzerland.
- [OpenTopoMap](https://opentopomap.org/): map tiles (CC-BY-SA).

## Build and release

You need [Rust](https://www.rust-lang.org/tools/install). On a Mac, `bar/bundle.sh` builds the three
programs and installs `Peakcast.app` in `/Applications`.

`./release 1.1.0` publishes a version: it sets the number, checks the code, tags and pushes, and
GitHub Actions builds the release and updates the Homebrew cask.

Progress is tracked in [issues](https://github.com/alrubearb-sys/peakcast/issues).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.

The panel and the map embed [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), the font
Omarchy uses, under the [SIL Open Font License 1.1](bar/ui/fonts/OFL.txt). The panel is made with
[Slint](https://slint.dev/).
