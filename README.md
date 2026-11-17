# Zerogrep

High-performance, zero-copy log diagnostic tool for Linux.

Zerogrep memory-maps log files and filters them in real time using
combinable regex/substring include-exclude rules with custom highlight
colors, so you can isolate the lines that matter and export them to a
new file. Built with Rust, GTK4, libadwaita and relm4.

## Features

- Zero-copy: files are memory-mapped, lines are indexed with SIMD scanning
- Filters: Regex / Substring, Include / Exclude, case-sensitive or not,
  custom highlight colors
- Per-filter context lines around matches (`C` both / `B` before / `A`
  after, grep-style)
- Highlight-only mode, descending (newest-first) order
- Save the visible (filtered) lines to a new file
- Filter sets: save, load, import and export named filter groups
- Status bar with the current file location

## Prerequisites

To build from source you need:

| Component    | Minimum version | Fedora package          |
|--------------|-----------------|-------------------------|
| Rust (stable)| 1.85 (edition 2024) | `rust`              |
| GTK4         | 4.14            | `gtk4-devel`            |
| libadwaita   | 1.5             | `libadwaita-devel`      |
| meson        | 0.62 (packaging only) | `meson`           |
| ninja        | any recent      | `ninja-build`           |

On Fedora 42+ / RHEL-family:

```sh
sudo dnf install rust gtk4-devel libadwaita-devel meson ninja-build
```

Other distributions: install the equivalent development packages
(`libgtk-4-dev`, `libadwaita-1-dev` on Debian/Ubuntu).

## Building

Quick way (no meson needed):

```sh
cargo run
```

Full build:

```sh
cargo build --release
```

Packaging layout via meson:

```sh
meson setup build -Drelease=true
ninja -C build
ninja -C build install
```

Run the engine tests with:

```sh
cargo test
```


## Usage

Launch the app and open a log file (`Ctrl+O`). Add filters with `Ctrl+N`
(double-click a filter to edit it, including colors and context lines).
`Ctrl+S` saves the filtered output, `Ctrl+C` copies the selected line,
`Ctrl+Q` quits.

## License

Apache-2.0
