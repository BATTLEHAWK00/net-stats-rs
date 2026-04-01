# net-stats-tui

[中文说明](README.zh-CN.md)

`net-stats-tui` is a terminal UI for monitoring live network throughput on Linux. It reads interface counters from `/sys/class/net/<iface>/statistics` and renders per-interface RX/TX rates in a table.

## Features

- Monitor multiple network interfaces with friendly aliases
- Compute RX/TX rates on a fixed refresh interval
- Use a small TOML config file
- Run entirely in the terminal

## Requirements

- Linux
- Rust 1.85+ (the project uses Edition 2024)
- Access to `/sys/class/net`

## Build From Source

```bash
cargo build --release
```

The compiled binary will be available at `target/release/net-stats-tui`.

## Configuration

Copy the sample config:

```bash
cp net-stats.toml.example net-stats.toml
```

Example configuration:

```toml
refresh_ms = 1000

[[interfaces]]
alias = "wan"
device = "eth0"

[[interfaces]]
alias = "wifi"
device = "wlan0"
```

Fields:

- `refresh_ms`: refresh interval in milliseconds, defaults to `1000`
- `interfaces[].alias`: label shown in the UI
- `interfaces[].device`: Linux interface name such as `eth0`, `wlan0`, or `enp3s0`

## Usage

By default, the app reads `net-stats.toml` from the current directory:

```bash
./target/release/net-stats-tui
```

You can also pass an explicit config path:

```bash
./target/release/net-stats-tui /path/to/net-stats.toml
```

At runtime:

- The table shows RX and TX rates for each configured interface
- If an interface cannot be read, that row shows an error message
- Press `q` to quit

## GitHub Release Automation

The repository includes a GitHub Actions workflow at `.github/workflows/release.yml`.

It will:

- Trigger when a tag matching `v*` is pushed
- Build the Linux binary with `cargo build --locked --release`
- Package `net-stats-tui-<tag>-x86_64-unknown-linux-gnu.tar.gz`
- Upload the archive to the matching GitHub Release

Release example:

```bash
git tag v0.1.0
git push origin v0.1.0
```

If the Release does not exist yet, the workflow creates it and enables generated release notes.

## Release Package Contents

Each archive contains:

- `net-stats-tui-<tag>-x86_64-unknown-linux-gnu`
- `net-stats.toml.example`
- `README.md`

## License

No license file is included yet.
