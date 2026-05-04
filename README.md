# tui2

`tui2` is a terminal UI launcher for other terminal UI apps. It detects common TUIs installed on your computer, lets you add your own commands, and prioritizes favorites and frequently used tools.

## Run

```bash
cargo run
```

## Controls

| Key | Action |
| --- | --- |
| `j` / Down | Move down |
| `k` / Up | Move up |
| Enter | Launch selected TUI |
| `a` | Add a custom launcher |
| `f` | Favorite or unfavorite selected launcher |
| `r` | Rescan detected and saved launchers |
| `q` / Esc | Quit |

## How Launchers Are Chosen

Launchers are sorted in this order:

1. Favorites
2. Most frequently used
3. Name

The list shows each launcher's source and usage, for example:

```text
[fav] Neovim  [auto | used 4x]
Lazygit       [auto | never used]
Notes         [user | used once]
```

## Config Files

Custom launchers are saved here:

```text
~/.config/tui2/launchers.txt
```

Format:

```text
name<TAB>command<TAB>description
```

Usage stats and favorites are saved here:

```text
~/.config/tui2/stats.txt
```

Format:

```text
command<TAB>launch_count<TAB>favorite
```

## Code Layout

| File | Purpose |
| --- | --- |
| `src/main.rs` | App startup |
| `src/app.rs` | App state, keyboard input, add flow, launching |
| `src/ui.rs` | Ratatui layout and rendering |
| `src/launcher.rs` | Launcher model, auto-detection, config, stats, sorting |
| `src/terminal.rs` | Terminal setup/restore and shell command execution |

To add or remove auto-detected programs, edit `known_tui_programs()` in `src/launcher.rs`.
