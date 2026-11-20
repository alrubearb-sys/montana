# montana

Should I go to the mountains this weekend?

`montana` is a small command-line tool, written in Rust, that checks the forecast for your
favourite ski and alpine areas and tells you which one looks best. Later it will also show a
summary in the [Omarchy](https://omarchy.org/) status bar.

> Status: early days. This is a learning project, so things will change and break.

## What it will do

```
montana bodele          # forecast for one area: new snow, wind, freezing level...
montana ranking sabado  # your areas, best to worst for Saturday
```

Two modes:

- **Ski:** new snow, snow line, wind at altitude.
- **Mountain** (climbing and alpinism): freezing level, daylight, storm risk, avalanche danger.

## Roadmap

1. CLI with the forecast for one area
2. Omarchy status bar plugin
3. Ranking of several areas
4. Optional: clickable map of Switzerland

Progress is tracked in [issues](https://github.com/alrubearb-sys/montana/issues) and
[milestones](https://github.com/alrubearb-sys/montana/milestones).

## Data sources

- [Open-Meteo](https://open-meteo.com/): forecast and geocoding, free and without an API key.
- [SLF](https://www.slf.ch/): Swiss avalanche bulletin (open data).

## Build

You need [Rust](https://www.rust-lang.org/tools/install).

```
cargo run
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.
