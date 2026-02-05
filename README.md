# fdmon

**File Descriptor Monitor** - A powerful Linux process file descriptor monitoring tool with both interactive TUI and CLI modes.

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Features

- 🖥️ **Interactive TUI Mode** - Real-time process monitoring with keyboard navigation
- 💻 **CLI Mode** - Scriptable subcommands for automation and integration
- 📊 **Multiple Output Formats** - Table, JSON, and CSV output
- 🔍 **Advanced Filtering** - Filter by user, command, PID, or FD count
- 🌲 **Process Tree View** - Visualize process hierarchies
- 📈 **System Statistics** - Track system-wide and per-user FD usage
- ⚡ **Fast & Efficient** - Built with Rust for performance

## Installation

### Quick Install (Recommended)

Install the latest version with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/ll931217/fdmon/master/install.sh | bash
```

This will automatically detect your platform and install the appropriate binary to `~/.local/bin/`.

### Manual Installation

Download the binary for your platform from the [releases page](https://github.com/ll931217/fdmon/releases):

**Linux (x86_64):**
```bash
curl -fsSL https://github.com/ll931217/fdmon/releases/latest/download/fdmon-linux-x86_64 -o fdmon
chmod +x fdmon
sudo mv fdmon /usr/local/bin/
```

**macOS (Intel):**
```bash
curl -fsSL https://github.com/ll931217/fdmon/releases/latest/download/fdmon-darwin-x86_64 -o fdmon
chmod +x fdmon
sudo mv fdmon /usr/local/bin/
```

**macOS (Apple Silicon):**
```bash
curl -fsSL https://github.com/ll931217/fdmon/releases/latest/download/fdmon-darwin-aarch64 -o fdmon
chmod +x fdmon
sudo mv fdmon /usr/local/bin/
```

### From Source

```bash
git clone https://github.com/ll931217/fdmon.git
cd fdmon
cargo build --release
sudo cp target/release/fdmon /usr/local/bin/
```

**Prerequisites:**
- Rust 1.70 or later
- Linux kernel 2.6.22+ (for /proc filesystem)

## Usage

### TUI Mode (Interactive)

Run without any subcommand to launch the interactive terminal UI:

```bash
fdmon
```

**Keyboard Controls:**
- `↑/↓` - Navigate processes
- `Tab` - Switch between Table/Tree view
- `/` - Search/filter
- `Enter` - View process details
- `K` - Kill process (with confirmation)
- `s` - Change sort column
- `+/-` - Adjust refresh interval
- `q` - Quit

### CLI Mode (Programmatic)

#### List Processes

```bash
# Basic list (top FD consumers)
fdmon list

# Filter and sort
fdmon list --filter chrome --sort fds --limit 10
fdmon list --user liangshih.lin --min-fds 50
fdmon list --sort command

# Different output formats
fdmon list --format json
fdmon list --format csv > processes.csv
```

#### Process Tree

```bash
# View process hierarchy (processes with FDs > 0 and their ancestors)
fdmon tree

# Export as JSON
fdmon tree --format json
```

#### Process Details

```bash
# Detailed FD information for specific process
fdmon detail 1234

# JSON output for parsing
fdmon detail 1234 --format json
```

#### System Statistics

```bash
# System-wide FD statistics
fdmon stats

# JSON format for monitoring
fdmon stats --format json
```

#### Top Processes

```bash
# Top 10 processes by FD count (default)
fdmon top

# Top 20
fdmon top 20
```

#### User Summary

```bash
# Per-user FD usage breakdown
fdmon summary

# CSV export
fdmon summary --format csv
```

## Output Formats

### Table (Default)

Human-readable aligned columns:

```
FDS      PID      OWNER        LIMIT    USAGE%   COMMAND
------------------------------------------------------------------------
256      12345    user1        524288   0.05     chrome
128      67890    user2        1024     12.50    node
```

### JSON

Machine-parseable structured data:

```json
[
  {
    "pid": 12345,
    "ppid": 1,
    "owner": "user1",
    "command": "chrome",
    "fds": 256,
    "limit": 524288,
    "usage_percent": 0.048828125
  }
]
```

### CSV

RFC 4180 compliant with proper escaping:

```csv
fds,pid,owner,limit,usage_percent,command
256,12345,user1,524288,0.05,chrome
128,67890,user2,1024,12.50,node
```

## Global Options

- `--format <FORMAT>` - Output format: `table`, `json`, or `csv` (default: `table`)
- `-i, --interval <SECONDS>` - Refresh interval for TUI mode (1-10 seconds, default: 2)
- `-c, --count <N>` - Exit TUI after N refreshes (default: 0 = run forever)

## Use Cases

### Debugging File Descriptor Leaks

```bash
# Monitor specific process over time
watch -n 1 'fdmon detail 1234 --format json | jq .fd_count'

# Find processes exceeding limits
fdmon list --min-fds 1000 --format json
```

### System Monitoring Integration

```bash
# Export metrics to monitoring system
fdmon stats --format json | curl -X POST -d @- http://monitoring-system/metrics

# Generate daily reports
fdmon summary --format csv >> /var/log/fd-usage-$(date +%Y%m%d).csv
```

### Automation Scripts

```bash
#!/bin/bash
# Alert if any process uses > 80% of FD limit
fdmon list --format json | jq -r '.[] | select(.usage_percent > 80) | "\(.command) (\(.pid)): \(.usage_percent)%"'
```

## Architecture

- **`src/proc.rs`** - Linux /proc filesystem parsing and data collection
- **`src/tree.rs`** - Process hierarchy tree building
- **`src/cli.rs`** - CLI subcommand execution and output formatting
- **`src/app.rs`** - TUI application state and event handling
- **`src/ui.rs`** - Terminal UI rendering (ratatui)
- **`src/event.rs`** - Keyboard event handling

## Dependencies

- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation
- [clap](https://github.com/clap-rs/clap) - Command-line argument parsing
- [serde](https://github.com/serde-rs/serde) - Serialization framework
- [chrono](https://github.com/chronotope/chrono) - Date and time library
- [nix](https://github.com/nix-rust/nix) - Unix system API
- [uzers](https://github.com/ogham/rust-users) - User/group information

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

### Running Tests

```bash
cargo test
```

### Building

```bash
cargo build --release
```

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Author

Liang-Shih Lin ([@ll931217](https://github.com/ll931217))

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Inspired by tools like `htop`, `lsof`, and `proc`
