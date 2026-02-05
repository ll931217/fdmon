# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-02-05

### Added
- Initial release of fdmon
- Interactive TUI mode with real-time process monitoring
- CLI mode with 6 subcommands:
  - `list` - List processes with FD counts (filterable, sortable)
  - `tree` - Show process hierarchy
  - `detail <PID>` - Show detailed FD information for specific process
  - `stats` - System-wide FD statistics
  - `top [N]` - Top N processes by FD count
  - `summary` - Per-user FD usage breakdown
- Multiple output formats:
  - Table (human-readable, default)
  - JSON (machine-parseable)
  - CSV (RFC 4180 compliant)
- Advanced filtering options:
  - Filter by user, command, PID
  - Minimum FD count threshold
  - Result limiting
- Sorting by FD count, PID, owner, or command
- Process tree visualization (FDs > 0 + ancestors)
- Real-time system and per-user FD statistics
- Keyboard-driven TUI with:
  - Navigation (↑/↓)
  - View switching (Tab)
  - Search/filter (/)
  - Process details (Enter)
  - Process kill with confirmation (K)
  - Sort column selection (s)
  - Refresh interval adjustment (+/-)
- Unit tests for CLI functionality (12 tests)
- Support for latest Rust dependencies:
  - ratatui 0.30.0
  - crossterm 0.29.0
  - nix 0.31.1

### Technical Details
- Linux /proc filesystem parsing
- Process hierarchy tree building
- Efficient data collection and caching
- Terminal UI with ratatui framework
- CSV escaping following RFC 4180
- Graceful error handling for permission denied and missing PIDs

[Unreleased]: https://github.com/ll931217/fdmon/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ll931217/fdmon/releases/tag/v0.1.0
