# SyncForge TUI

A cross-platform terminal user interface for database schema comparison and data synchronization.

## Features

- **Multi-Database Support**: MySQL, PostgreSQL, SQLite, SQL Server
- **Connection Management**: Save and manage multiple database connections
- **Schema Diff**: Compare table structures between two databases
- **Data Sync**: Compare and synchronize data between databases with INSERT/UPDATE/DELETE detection
- **Table Browser**: Browse table data with pagination and horizontal column scrolling
- **SQL Preview**: View generated SQL statements before execution

## Screenshots

```
┌─ F1 Connections ─┬─ F2 Schema Diff ─┬─ F3 Data Sync ─┬─ F4 Browser ─┐
│                                                                     │
│  ┌─ Tables (5) ─────────┐  ┌─ users [Col 1-5/8] ─────────────────┐ │
│  │ users                │  │ id │ name │ email │ created │ ...   │ │
│  │ orders               │  │ 1  │ John │ j@... │ 2024-01 │       │ │
│  │ products             │  │ 2  │ Jane │ ja@.. │ 2024-02 │       │ │
│  └──────────────────────┘  └─────────────────────────────────────┘ │
│                                                                     │
│  [Ctrl+L]Load [Enter]View [←→]Page [Shift+←→]Cols [Tab]Focus       │
└─────────────────────────────────────────────────────────────────────┘
```

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/syncforge-tui.git
cd syncforge-tui

# Build
cargo build --release

# Run
cargo run --release
```

## Keyboard Shortcuts

### Global
| Key | Action |
|-----|--------|
| `F1-F4` | Switch tabs |
| `Esc` | Quit / Go back |
| `Tab` | Toggle focus between panels |

### Connection Form (F1)
| Key | Action |
|-----|--------|
| `↑/↓` | Navigate fields |
| `Enter` | Edit field / Test connection |
| `Ctrl+S` | Save connection |
| `Ctrl+T` | Test connection |

### Schema Diff (F2)
| Key | Action |
|-----|--------|
| `Enter` | Compare schemas |
| `↑/↓` | Navigate differences |

### Data Sync (F3)
| Key | Action |
|-----|--------|
| `Ctrl+L` | Load tables |
| `Enter` | Compare table data |
| `Tab` | Toggle focus |
| `↑/↓` | Navigate |

### Table Browser (F4)
| Key | Action |
|-----|--------|
| `Ctrl+L` | Load tables |
| `Enter` | Load table data |
| `←/→` | Previous/Next page |
| `Shift+←/→` | Scroll columns left/right |
| `↑/↓` | Navigate rows |
| `Tab` | Toggle focus |

## Configuration

Connections are saved in:
- **macOS**: `~/Library/Application Support/syncforge-tui/connections.json`
- **Linux**: `~/.config/syncforge-tui/connections.json`
- **Windows**: `%APPDATA%\syncforge-tui\connections.json`

## Requirements

- Rust 1.70+
- For SQL Server: TDS 7.3 protocol support

## License

MIT
