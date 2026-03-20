```
   █████╗ ███╗   ███╗███████╗██╗  ██╗
  ██╔══██╗████╗ ████║██╔════╝██║ ██╔╝
  ███████║██╔████╔██║█████╗  █████╔╝
  ██╔══██║██║╚██╔╝██║██╔══╝  ██╔═██╗
  ██║  ██║██║ ╚═╝ ██║███████╗██║  ██╗
  ╚═╝  ╚═╝╚═╝     ╚═╝╚══════╝╚═╝  ╚═╝
```
**A terminal IDE written in Rust.**  
No Electron. No LSP daemon. No config files. Just a single file.

---

## Features

- **Modal editing** — Normal · Insert · Visual · Command modes
- **Multi-tab** — open many files at once, switch instantly
- **File Explorer** — sidebar tree, navigate and open files
- **Embedded Terminal** — run shell commands without leaving the editor
- **Syntax highlighting** — 8 languages out of the box
- **Git status** — branch, modified, staged, untracked shown on the dashboard
- **Startup dashboard** — LazyVim-style welcome screen with file list
- **Unicode UI** — `╭─╮ │ ╰─╯` borders, `▔` tab indicators, `●` dirty markers
- **Zero dependencies** at runtime — one binary, runs anywhere

---

## Installation

### Build from source

```bash
git clone https://github.com/yourname/amek
cd amek
cargo build --release
# Binary is at target/release/amek
```

**Requirements:** Rust 1.75+ · Linux / macOS · a terminal with 256-colour support

### Quick install (copy binary)

```bash
cp target/release/amek ~/.local/bin/amek
chmod +x ~/.local/bin/amek
```

---

## Usage

```bash
amek                  # open dashboard
amek file.rs          # open a file directly
amek src/main.rs      # path works too
```

---

## Interface

```
┌─ Title bar ──────────────────────────────────── [ NORMAL ] Ln 1  Col 1 ─┐
│ tab1.rs  ●  │  tab2.py  │              ^B new  ^M close  ^← ^→ switch   │  ← Tab bar
├─ Explorer ──┬─ Editor ──────────────────────────────────────────────────┤
│ ▸ /home/..  │    1  fn main() {                                          │
│ · main.rs   │    2      println!("hello");                               │
│ · Cargo.tom │    3  }                                                    │
│             │  ~                                                          │
│             │  ~                                                          │
├─────────────┴────────────────────────────────────────────────────────────┤
│ ❯ TERMINAL                                                               │  ← Terminal pane
│ ❯ cargo build                                                            │    (c to open)
│   Compiling amek v0.2.0                                                  │
│   Finished release [optimized]                                           │
│ ❯ ▌                                                                      │
├──────────────────────────────────────────────────────────────────────────┤
│  AMEK  |  i=insert  v=visual ...                                    RS   │  ← Status bar
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Keybindings

### Dashboard (startup screen)

| Key | Action |
|-----|--------|
| `↑` / `↓` or `j` / `k` | Navigate file list |
| `Enter` or `o` | Open highlighted file |
| `n` | New empty file |
| `e` | Open file explorer |
| `?` | Help |
| `q` | Quit |

---

### Normal Mode

| Key | Action |
|-----|--------|
| `i` | Enter Insert mode |
| `v` | Enter Visual mode |
| `e` / `Tab` | Focus File Explorer |
| `c` | Open Terminal panel |
| `:` | Enter Command mode |
| `?` | Help screen |
| `↑ ↓ ← →` | Move cursor |
| `Home` / `End` | Start / end of line |
| `PgUp` / `PgDn` | Page up / page down |
| `Ctrl+S` | Save file |
| `Ctrl+Q` | Quit |

---

### Insert Mode

| Key | Action |
|-----|--------|
| `Esc` | Return to Normal mode |
| `Enter` | New line |
| `Backspace` | Delete previous character |
| `Delete` | Delete next character |
| `Tab` | Insert 4 spaces |
| `Ctrl+S` | Save file |

---

### Visual Mode

| Key | Action |
|-----|--------|
| `↑ ↓ ← →` | Extend selection |
| `d` | Delete selection |
| `y` | Yank (copy) selection |
| `i` | Delete selection and enter Insert |
| `Esc` | Cancel selection |

---

### Tabs

| Key | Action |
|-----|--------|
| `Ctrl+B` | New tab |
| `Ctrl+M` | Close current tab |
| `Ctrl+←` | Previous tab |
| `Ctrl+→` | Next tab |

Works in both Normal and Insert mode. Opening a file always creates a new tab.

---

### File Explorer

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate entries |
| `Enter` | Open file / enter directory |
| `Backspace` | Go up one directory |
| `Esc` | Return to editor |

---

### Terminal Panel (`c` to open)

| Key | Action |
|-----|--------|
| Type | Enter shell command |
| `Enter` | Execute command |
| `Backspace` | Delete input character |
| `Esc` | Return to editor |

Special built-in commands:

| Command | Effect |
|---------|--------|
| `cd <dir>` | Change working directory |
| `clear` | Clear terminal output |

---

### Command Mode (`:`)

| Command | Action |
|---------|--------|
| `:w` | Save current file |
| `:q` | Quit (warns if unsaved) |
| `:wq` | Save and quit |
| `:q!` | Force quit without saving |
| `:new` | New empty tab |
| `:open <path>` | Open file by path in new tab |
| `:saveas <path>` | Save buffer to a new path |
| `:explorer` | Toggle Explorer panel |
| `:term` | Toggle Terminal panel |
| `:tabnew` / `:tn` | New tab |
| `:tabclose` / `:tc` | Close current tab |
| `:tabnext` / `:tbn` | Next tab |
| `:tabprev` / `:tbp` | Previous tab |

---

### Help Screen (`?`)

Navigate between sections with `←` `→`. Close with `Esc`.

Sections: Normal Mode · Insert Mode · Visual Mode · Explorer · Terminal · Commands · Syntax Highlight

---

## Syntax Highlighting

| Language | Extensions |
|----------|------------|
| Rust | `.rs` |
| C | `.c` `.h` |
| C++ | `.cpp` `.cc` `.cxx` `.hpp` |
| HTML | `.html` `.htm` |
| CSS | `.css` |
| JavaScript / TypeScript | `.js` `.ts` `.jsx` `.tsx` |
| Python | `.py` |
| Lua | `.lua` |

Colour palette is VS Code Dark+ inspired: keywords blue, strings orange, comments green, types teal, macros yellow, numbers light green.

---

## Git Integration

The dashboard reads git status on startup from the current working directory:

- **Branch name**
- **Modified files** — unstaged changes
- **Staged files** — files in the index
- **Untracked files**
- **Ahead / behind** — commits ahead or behind the upstream

No git repository? The panel is hidden automatically.

---

## Architecture

The entire editor is a single Rust file with zero runtime dependencies beyond the standard library and `crossterm` for terminal I/O.

```
src/main.rs
│
├── Syntax highlighting   — tokenisers for each language
├── GitStatus            — runs git CLI, parses porcelain output
├── Dashboard            — startup screen with logo + file list
├── Explorer             — directory tree sidebar
├── Buffer               — text storage, cursor, char↔byte indexing
├── TermPane             — embedded shell (sh -c)
├── Help                 — tabbed help viewer
└── App                  — modal editor, tab manager, renderer
```

**Key design decisions:**

- `cursor_col` is always a **character index**, not a byte index. All string operations convert to byte offsets via `char_indices()` before touching `String` methods. This makes the editor correct for UTF-8 text.
- All key events are filtered to `KeyEventKind::Press` only, preventing ghost characters from terminal key-release events.
- Backspace is matched as both `KeyCode::Backspace` and `KeyCode::Char('\x7f')` (the DEL byte that many Linux terminals send) to work across terminal emulators.

---

## Dependencies

```toml
[dependencies]
crossterm = "0.27"
```

That's it.

---

## License

GPL version 3
