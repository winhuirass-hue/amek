use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{self, Color, Stylize},
    terminal::{self, ClearType},
};
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, PartialEq)]
enum TokenKind {
    Keyword, String_, Comment, Number, Type_, Macro, Normal, Punctuation,
}

struct Token { text: String, kind: TokenKind }

fn highlight_line(line: &str, ext: &str) -> Vec<Token> {
    match ext {
        "rs" => tokenize(line,
            &["fn","let","mut","pub","use","mod","struct","enum","impl","trait","if","else",
              "match","for","while","loop","return","self","super","crate","in","as","where",
              "type","const","static","async","await","move","ref","dyn","true","false"],
            &["i8","i16","i32","i64","i128","isize","u8","u16","u32","u64","u128","usize",
              "f32","f64","bool","char","str","String","Vec","Option","Result","Box","Arc","Rc"]),
        "py" => tokenize(line,
            &["def","class","if","elif","else","for","while","return","import","from","as",
              "with","try","except","finally","raise","pass","break","continue","lambda",
              "yield","and","or","not","in","is","True","False","None"],
            &["int","float","str","bool","list","dict","tuple","set","bytes","type","object"]),
        "js"|"ts" => tokenize(line,
            &["const","let","var","function","return","if","else","for","while","class",
              "extends","new","this","import","export","default","async","await","typeof",
              "instanceof","true","false","null","undefined","switch","case","break"],
            &["Number","String","Boolean","Array","Object","Promise","Map","Set","Error"]),
        "c"|"cpp"|"h" => tokenize(line,
            &["int","char","float","double","void","if","else","for","while","do","return",
              "struct","enum","typedef","union","switch","case","break","continue","static",
              "extern","const","sizeof"],
            &["size_t","uint8_t","uint32_t","bool","FILE","NULL"]),
        _ => vec![Token { text: line.to_string(), kind: TokenKind::Normal }],
    }
}

fn tokenize(line: &str, keywords: &[&str], types: &[&str]) -> Vec<Token> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('#') {
        return vec![Token { text: line.to_string(), kind: TokenKind::Comment }];
    }
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut tokens = Vec::new();
    while i < len {
        if chars[i] == '"' || chars[i] == '\'' {
            let q = chars[i];
            let mut s = String::from(q);
            i += 1;
            while i < len {
                s.push(chars[i]);
                if chars[i] == q { i += 1; break; }
                i += 1;
            }
            tokens.push(Token { text: s, kind: TokenKind::String_ });
        } else if chars[i].is_ascii_digit() {
            let mut s = String::new();
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                s.push(chars[i]); i += 1;
            }
            tokens.push(Token { text: s, kind: TokenKind::Number });
        } else if chars[i].is_alphabetic() || chars[i] == '_' {
            let mut w = String::new();
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                w.push(chars[i]); i += 1;
            }
            let is_macro = i < len && chars[i] == '!';
            if is_macro { w.push('!'); i += 1; tokens.push(Token { text: w, kind: TokenKind::Macro }); }
            else if keywords.contains(&w.as_str()) { tokens.push(Token { text: w, kind: TokenKind::Keyword }); }
            else if types.contains(&w.as_str()) { tokens.push(Token { text: w, kind: TokenKind::Type_ }); }
            else { tokens.push(Token { text: w, kind: TokenKind::Normal }); }
        } else if "{}()[];,.<>|&*+-=!".contains(chars[i]) {
            tokens.push(Token { text: chars[i].to_string(), kind: TokenKind::Punctuation }); i += 1;
        } else {
            tokens.push(Token { text: chars[i].to_string(), kind: TokenKind::Normal }); i += 1;
        }
    }
    tokens
}

fn token_color(k: &TokenKind) -> Color {
    match k {
        TokenKind::Keyword     => Color::Rgb { r: 86,  g: 156, b: 214 },
        TokenKind::String_     => Color::Rgb { r: 206, g: 145, b: 120 },
        TokenKind::Comment     => Color::Rgb { r: 106, g: 153, b: 85  },
        TokenKind::Number      => Color::Rgb { r: 181, g: 206, b: 168 },
        TokenKind::Type_       => Color::Rgb { r: 78,  g: 201, b: 176 },
        TokenKind::Macro       => Color::Rgb { r: 220, g: 220, b: 170 },
        TokenKind::Punctuation => Color::Rgb { r: 200, g: 200, b: 200 },
        TokenKind::Normal      => Color::Rgb { r: 212, g: 212, b: 212 },
    }
}

#[derive(PartialEq, Clone)]
enum Mode { Normal, Insert, Explorer, Command, Help }

struct Explorer {
    dir: PathBuf,
    entries: Vec<PathBuf>,
    selected: usize,
    scroll: usize,
}

impl Explorer {
    fn new(dir: PathBuf) -> Self {
        let mut e = Explorer { dir, entries: vec![], selected: 0, scroll: 0 };
        e.refresh(); e
    }
    fn refresh(&mut self) {
        self.entries.clear();
        if let Ok(rd) = fs::read_dir(&self.dir) {
            let mut dirs = Vec::new(); let mut files = Vec::new();
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() { dirs.push(p); } else { files.push(p); }
            }
            dirs.sort(); files.sort();
            self.entries.extend(dirs); self.entries.extend(files);
        }
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }
    fn go_up(&mut self) {
        if let Some(parent) = self.dir.parent() {
            self.dir = parent.to_path_buf(); self.selected = 0; self.scroll = 0; self.refresh();
        }
    }
    fn enter(&mut self) -> Option<PathBuf> {
        if let Some(p) = self.entries.get(self.selected) {
            if p.is_dir() {
                self.dir = p.clone(); self.selected = 0; self.scroll = 0; self.refresh(); None
            } else { Some(p.clone()) }
        } else { None }
    }
}

struct Buffer {
    lines: Vec<String>,
    path: Option<PathBuf>,
    dirty: bool,
    cursor_row: usize, cursor_col: usize,
    scroll_row: usize, scroll_col: usize,
}

impl Buffer {
    fn empty() -> Self {
        Buffer { lines: vec![String::new()], path: None, dirty: false,
                 cursor_row: 0, cursor_col: 0, scroll_row: 0, scroll_col: 0 }
    }
    fn from_file(path: PathBuf) -> io::Result<Self> {
        let content = fs::read_to_string(&path)?;
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else { content.lines().map(|l| l.to_string()).collect() };
        Ok(Buffer { lines, path: Some(path), dirty: false,
                    cursor_row: 0, cursor_col: 0, scroll_row: 0, scroll_col: 0 })
    }
    fn save(&mut self) -> io::Result<()> {
        if let Some(ref p) = self.path {
            fs::write(p, self.lines.join("\n"))?; self.dirty = false;
        }
        Ok(())
    }
    fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
        self.path = Some(path); self.save()
    }
    /// Convert a char-index to a byte-index for a given line string.
    fn char_to_byte(line: &str, char_idx: usize) -> usize {
        line.char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(line.len())
    }

    fn insert_char(&mut self, c: char) {
        let (row, col) = (self.cursor_row, self.cursor_col);
        if row >= self.lines.len() { self.lines.push(String::new()); }
        let char_count = self.lines[row].chars().count();
        let col = col.min(char_count);
        let byte_idx = Self::char_to_byte(&self.lines[row], col);
        self.lines[row].insert(byte_idx, c);
        self.cursor_col += 1; self.dirty = true;
    }
    fn insert_newline(&mut self) {
        let row = self.cursor_row;
        let char_count = self.lines[row].chars().count();
        let col = self.cursor_col.min(char_count);
        let byte_idx = Self::char_to_byte(&self.lines[row], col);
        let rest = self.lines[row].split_off(byte_idx);
        self.lines.insert(row + 1, rest);
        self.cursor_row += 1; self.cursor_col = 0; self.dirty = true;
    }
    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let (row, col) = (self.cursor_row, self.cursor_col);
            let byte_idx = Self::char_to_byte(&self.lines[row], col - 1);
            self.lines[row].remove(byte_idx);
            self.cursor_col -= 1; self.dirty = true;
        } else if self.cursor_row > 0 {
            let row = self.cursor_row;
            let cur = self.lines.remove(row);
            let prev_char_len = self.lines[row - 1].chars().count();
            self.lines[row - 1].push_str(&cur);
            self.cursor_row -= 1; self.cursor_col = prev_char_len; self.dirty = true;
        }
    }
    fn delete_char(&mut self) {
        let (row, col) = (self.cursor_row, self.cursor_col);
        let char_count = self.lines[row].chars().count();
        if col < char_count {
            let byte_idx = Self::char_to_byte(&self.lines[row], col);
            self.lines[row].remove(byte_idx); self.dirty = true;
        } else if row + 1 < self.lines.len() {
            let next = self.lines.remove(row + 1);
            self.lines[row].push_str(&next); self.dirty = true;
        }
    }
    fn move_cursor(&mut self, dr: i32, dc: i32) {
        self.cursor_row = (self.cursor_row as i32 + dr)
            .clamp(0, self.lines.len() as i32 - 1) as usize;
        let char_len = self.lines[self.cursor_row].chars().count() as i32;
        if dc != 0 {
            self.cursor_col = (self.cursor_col as i32 + dc).clamp(0, char_len) as usize;
        } else {
            self.cursor_col = self.cursor_col.min(char_len as usize);
        }
    }
    fn file_ext(&self) -> String {
        self.path.as_ref()
            .and_then(|p| p.extension()).and_then(|e| e.to_str())
            .unwrap_or("").to_lowercase()
    }
}

const EXPLORER_W: u16 = 28;

struct App {
    mode: Mode,
    buffer: Buffer,
    explorer: Explorer,
    status_msg: String,
    command_buf: String,
    term_w: u16, term_h: u16,
    show_explorer: bool,
}

impl App {
    fn new(initial_path: Option<PathBuf>) -> io::Result<Self> {
        let (w, h) = terminal::size()?;
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let buffer = if let Some(p) = &initial_path {
            Buffer::from_file(p.clone()).unwrap_or_else(|_| Buffer::empty())
        } else { Buffer::empty() };
        Ok(App {
            mode: Mode::Normal, buffer,
            explorer: Explorer::new(cwd),
            status_msg: "Welcome to AMEK  |  i=insert  e=explorer  :=command  ?=help".into(),
            command_buf: String::new(),
            term_w: w, term_h: h, show_explorer: true,
        })
    }

    fn render(&mut self, out: &mut impl Write) -> io::Result<()> {
        queue!(out, terminal::Clear(ClearType::All))?;
        self.render_titlebar(out)?;
        if self.show_explorer { self.render_explorer(out)?; }
        self.render_editor(out)?;
        self.render_statusbar(out)?;
        if self.mode == Mode::Command { self.render_cmdline(out)?; }
        self.position_cursor(out)?;
        out.flush()
    }

    fn render_titlebar(&self, out: &mut impl Write) -> io::Result<()> {
        let fname = self.buffer.path.as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "[untitled]".into());
        let dirty = if self.buffer.dirty { " *" } else { "" };
        let left = format!("  AMEK  |  {}{}", fname, dirty);
        let mode_lbl = match self.mode {
            Mode::Normal   => " NORMAL ",
            Mode::Insert   => " INSERT ",
            Mode::Explorer => " EXPLOR ",
            Mode::Command  => " COMAND ",
            Mode::Help     => " HELP   ",
        };
        let mode_bg = match self.mode {
            Mode::Insert   => Color::Rgb { r: 40, g: 160, b: 80 },
            Mode::Explorer => Color::Rgb { r: 30, g: 130, b: 180 },
            Mode::Command  => Color::Rgb { r: 160, g: 120, b: 20 },
            _              => Color::Rgb { r: 40, g: 70, b: 140 },
        };
        let right = format!("{} Ln{} Col{} ", mode_lbl,
            self.buffer.cursor_row + 1, self.buffer.cursor_col + 1);
        let fill = (self.term_w as usize).saturating_sub(left.len() + right.len());
        let bar: String = format!("{}{}{}", left, " ".repeat(fill), right)
            .chars().take(self.term_w as usize).collect();
        queue!(out,
            cursor::MoveTo(0, 0),
            style::SetBackgroundColor(Color::Rgb { r: 15, g: 20, b: 40 }),
            style::SetForegroundColor(Color::Rgb { r: 160, g: 180, b: 220 }),
            style::Print(&bar),
            style::SetBackgroundColor(Color::Reset),
            style::SetForegroundColor(Color::Reset),
        )?;
        // Mode badge
        let badge_x = (self.term_w as usize).saturating_sub(right.len()) as u16;
        queue!(out,
            cursor::MoveTo(badge_x, 0),
            style::SetBackgroundColor(mode_bg),
            style::SetForegroundColor(Color::Black),
            style::Print(mode_lbl),
            style::SetBackgroundColor(Color::Reset),
            style::SetForegroundColor(Color::Reset),
        )
    }

    fn render_explorer(&mut self, out: &mut impl Write) -> io::Result<()> {
        let h = self.term_h - 2;
        let w = EXPLORER_W as usize;
        for row in 0..h {
            queue!(out, cursor::MoveTo(0, row + 1),
                style::SetBackgroundColor(Color::Rgb { r: 18, g: 20, b: 28 }),
                style::Print(" ".repeat(w)),
                style::SetBackgroundColor(Color::Reset),
            )?;
        }
        // Header
        let dir_label = {
            let d = self.explorer.dir.to_string_lossy();
            let trimmed = if d.len() > w - 3 { &d[d.len() - (w - 3)..] } else { &d };
            format!(" > {}", trimmed)
        };
        let header = pad_str(&dir_label, w);
        queue!(out, cursor::MoveTo(0, 1),
            style::SetBackgroundColor(Color::Rgb { r: 28, g: 32, b: 48 }),
            style::SetForegroundColor(Color::Rgb { r: 100, g: 200, b: 220 }),
            style::Print(header),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        let visible = (h - 2) as usize;
        if self.explorer.selected < self.explorer.scroll { self.explorer.scroll = self.explorer.selected; }
        else if self.explorer.selected >= self.explorer.scroll + visible {
            self.explorer.scroll = self.explorer.selected - visible + 1;
        }
        for (i, entry) in self.explorer.entries.iter().enumerate()
            .skip(self.explorer.scroll).take(visible)
        {
            let row = (i - self.explorer.scroll + 2) as u16 + 1;
            let is_dir = entry.is_dir();
            let name = entry.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let prefix = if is_dir { "/ " } else { "  " };
            let label = pad_str(&format!(" {}{}", prefix, name), w);
            let selected = i == self.explorer.selected;
            let (bg, fg) = if selected {
                (Color::Rgb { r: 40, g: 80, b: 160 }, Color::White)
            } else if is_dir {
                (Color::Rgb { r: 18, g: 20, b: 28 }, Color::Rgb { r: 100, g: 180, b: 220 })
            } else {
                (Color::Rgb { r: 18, g: 20, b: 28 }, Color::Rgb { r: 170, g: 170, b: 180 })
            };
            queue!(out, cursor::MoveTo(0, row),
                style::SetBackgroundColor(bg), style::SetForegroundColor(fg),
                style::Print(label),
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
            )?;
        }
        // Separator
        for row in 0..h {
            queue!(out, cursor::MoveTo(EXPLORER_W, row + 1),
                style::SetForegroundColor(Color::Rgb { r: 40, g: 50, b: 80 }),
                style::Print("|"),
                style::SetForegroundColor(Color::Reset),
            )?;
        }
        Ok(())
    }

    fn render_editor(&mut self, out: &mut impl Write) -> io::Result<()> {
        let ex = if self.show_explorer { EXPLORER_W + 1 } else { 0 };
        let ew = self.term_w.saturating_sub(ex) as usize;
        let eh = (self.term_h - 2) as usize;
        let gutter = 5usize;
        let ext = self.buffer.file_ext();
        // Scroll
        if self.buffer.cursor_row < self.buffer.scroll_row {
            self.buffer.scroll_row = self.buffer.cursor_row;
        } else if self.buffer.cursor_row >= self.buffer.scroll_row + eh {
            self.buffer.scroll_row = self.buffer.cursor_row - eh + 1;
        }
        let col_area = ew.saturating_sub(gutter);
        if self.buffer.cursor_col < self.buffer.scroll_col {
            self.buffer.scroll_col = self.buffer.cursor_col;
        } else if self.buffer.cursor_col >= self.buffer.scroll_col + col_area {
            self.buffer.scroll_col = self.buffer.cursor_col - col_area + 1;
        }
        for sr in 0..eh {
            let br = sr + self.buffer.scroll_row;
            queue!(out, cursor::MoveTo(ex, sr as u16 + 1))?;
            if br < self.buffer.lines.len() {
                // Gutter
                let gc = if br == self.buffer.cursor_row {
                    Color::Rgb { r: 220, g: 180, b: 60 }
                } else { Color::Rgb { r: 60, g: 65, b: 80 } };
                queue!(out, style::SetForegroundColor(gc),
                    style::Print(format!("{:>4} ", br + 1)),
                    style::SetForegroundColor(Color::Reset),
                )?;
                // Syntax highlight
                let line = &self.buffer.lines[br];
                let tokens = highlight_line(line, &ext);
                let sc = self.buffer.scroll_col;
                let mut cp = 0usize;
                for tok in &tokens {
                    let ts = cp; let te = cp + tok.text.len(); cp = te;
                    let ve = sc + col_area;
                    if te <= sc || ts >= ve { continue; }
                    let clipped: String = tok.text.chars().enumerate()
                        .filter(|(i, _)| { let a = ts + i; a >= sc && a < ve })
                        .map(|(_, c)| c).collect();
                    queue!(out, style::SetForegroundColor(token_color(&tok.kind)),
                        style::Print(&clipped),
                        style::SetForegroundColor(Color::Reset),
                    )?;
                }
                let visible_len = line.chars().skip(sc).take(col_area).count();
                let pad = col_area.saturating_sub(visible_len);
                queue!(out, style::Print(" ".repeat(pad)))?;
            } else {
                queue!(out,
                    style::SetForegroundColor(Color::Rgb { r: 40, g: 45, b: 65 }),
                    style::Print(format!("{:>4} ", "~")),
                    style::SetForegroundColor(Color::Reset),
                    style::Print(" ".repeat(col_area)),
                )?;
            }
        }
        Ok(())
    }

    fn render_statusbar(&self, out: &mut impl Write) -> io::Result<()> {
        let y = self.term_h - 1;
        let left = format!("  {}  ", self.status_msg);
        let ext = self.buffer.file_ext().to_uppercase();
        let right = format!("  {}  ", if ext.is_empty() { "TXT".into() } else { ext });
        let fill = (self.term_w as usize).saturating_sub(left.len() + right.len());
        let bar: String = format!("{}{}{}", left, " ".repeat(fill), right)
            .chars().take(self.term_w as usize).collect();
        queue!(out, cursor::MoveTo(0, y),
            style::SetBackgroundColor(Color::Rgb { r: 20, g: 28, b: 50 }),
            style::SetForegroundColor(Color::Rgb { r: 140, g: 160, b: 210 }),
            style::Print(bar),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )
    }

    fn render_cmdline(&self, out: &mut impl Write) -> io::Result<()> {
        let y = self.term_h - 1;
        let content = format!(":{}{}", self.command_buf,
            " ".repeat((self.term_w as usize).saturating_sub(self.command_buf.len() + 1)));
        queue!(out, cursor::MoveTo(0, y),
            style::SetBackgroundColor(Color::Rgb { r: 8, g: 10, b: 18 }),
            style::SetForegroundColor(Color::Rgb { r: 220, g: 200, b: 80 }),
            style::Print(content),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )
    }

    fn render_help(&mut self, out: &mut impl Write) -> io::Result<()> {
        queue!(out, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0),
            style::SetForegroundColor(Color::Rgb { r: 80, g: 180, b: 220 }),
        )?;
        let lines = [
            "",
            "  +---------------------------------------------------------+",
            "  |            AMEK  -  Terminal IDE in Rust                |",
            "  +---------------------------------------------------------+",
            "  |  NORMAL MODE                                            |",
            "  |    i          Enter Insert mode                         |",
            "  |    e / Tab    Toggle & focus file explorer              |",
            "  |    :          Enter command mode                        |",
            "  |    ?          This help screen                          |",
            "  |    Arrows     Move cursor                               |",
            "  |    Ctrl+S     Save file                                 |",
            "  |    Ctrl+Q     Quit                                      |",
            "  |                                                         |",
            "  |  INSERT MODE                                            |",
            "  |    Esc        Return to Normal mode                     |",
            "  |    Enter      New line                                  |",
            "  |    Backspace  Delete previous character                 |",
            "  |    Delete     Delete next character                     |",
            "  |    Tab        Insert 4 spaces                           |",
            "  |                                                         |",
            "  |  FILE EXPLORER                                          |",
            "  |    Up/Down    Navigate entries                          |",
            "  |    Enter      Open file / enter directory               |",
            "  |    Backspace  Go up one directory                       |",
            "  |    Esc        Back to editor                            |",
            "  |                                                         |",
            "  |  COMMANDS  (type after :)                               |",
            "  |    :w          Save current file                        |",
            "  |    :q          Quit (warns on unsaved)                  |",
            "  |    :wq         Save and quit                            |",
            "  |    :q!         Force quit without saving                |",
            "  |    :new        New empty buffer                         |",
            "  |    :open <f>   Open a file by path                      |",
            "  |    :saveas <f> Save buffer to new path                  |",
            "  |    :explorer   Toggle explorer panel                    |",
            "  +---------------------------------------------------------+",
            "",
            "  Press any key to continue...",
        ];
        for ln in &lines {
            queue!(out, style::Print(ln), style::Print("\r\n"))?;
        }
        queue!(out, style::SetForegroundColor(Color::Reset))?;
        out.flush()?;
        event::read()?;
        self.mode = Mode::Normal;
        Ok(())
    }

    fn position_cursor(&self, out: &mut impl Write) -> io::Result<()> {
        match self.mode {
            Mode::Explorer => {
                let row = (self.explorer.selected.saturating_sub(self.explorer.scroll) + 2) as u16 + 1;
                queue!(out, cursor::MoveTo(1, row))
            }
            Mode::Command => {
                queue!(out, cursor::MoveTo(self.command_buf.len() as u16 + 1, self.term_h - 1))
            }
            _ => {
                let ex = if self.show_explorer { EXPLORER_W + 1 } else { 0 };
                let x = ex + 5 + (self.buffer.cursor_col.saturating_sub(self.buffer.scroll_col)) as u16;
                let y = 1 + (self.buffer.cursor_row.saturating_sub(self.buffer.scroll_row)) as u16;
                queue!(out, cursor::MoveTo(x, y))
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.mode {
            Mode::Normal   => self.handle_normal(key),
            Mode::Insert   => self.handle_insert(key),
            Mode::Explorer => self.handle_explorer(key),
            Mode::Command  => self.handle_command(key),
            Mode::Help     => { self.mode = Mode::Normal; false }
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.status_msg = match self.buffer.save() {
                    Ok(_) => "Saved.".into(), Err(e) => format!("Error: {}", e),
                };
            }
            (KeyCode::Char('i'), _) => { self.mode = Mode::Insert; self.status_msg = "-- INSERT --".into(); }
            (KeyCode::Char('e'), _) | (KeyCode::Tab, _) => {
                self.show_explorer = true; self.mode = Mode::Explorer;
                self.status_msg = "Explorer: Enter=open  Bksp=up  Esc=back".into();
            }
            (KeyCode::Char(':'), _) => { self.mode = Mode::Command; self.command_buf.clear(); }
            (KeyCode::Char('?'), _) => { self.mode = Mode::Help; }
            (KeyCode::Up, _)       => self.buffer.move_cursor(-1, 0),
            (KeyCode::Down, _)     => self.buffer.move_cursor(1, 0),
            (KeyCode::Left, _)     => self.buffer.move_cursor(0, -1),
            (KeyCode::Right, _)    => self.buffer.move_cursor(0, 1),
            (KeyCode::Home, _)     => self.buffer.cursor_col = 0,
            (KeyCode::End, _)      => { let r = self.buffer.cursor_row; self.buffer.cursor_col = self.buffer.lines[r].len(); }
            (KeyCode::PageUp, _)   => self.buffer.move_cursor(-((self.term_h - 2) as i32), 0),
            (KeyCode::PageDown, _) => self.buffer.move_cursor((self.term_h - 2) as i32, 0),
            _ => {}
        }
        false
    }

    fn handle_insert(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _)                           => { self.mode = Mode::Normal; self.status_msg = "NORMAL".into(); }
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => { self.status_msg = match self.buffer.save() { Ok(_) => "Saved.".into(), Err(e) => format!("Error: {}", e) }; }
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Enter, _)                         => self.buffer.insert_newline(),
            (KeyCode::Backspace, _)                     => self.buffer.backspace(),
            (KeyCode::Delete, _)                        => self.buffer.delete_char(),
            (KeyCode::Up, _)                            => self.buffer.move_cursor(-1, 0),
            (KeyCode::Down, _)                          => self.buffer.move_cursor(1, 0),
            (KeyCode::Left, _)                          => self.buffer.move_cursor(0, -1),
            (KeyCode::Right, _)                         => self.buffer.move_cursor(0, 1),
            (KeyCode::Home, _)                          => self.buffer.cursor_col = 0,
            (KeyCode::End, _)                           => { let r = self.buffer.cursor_row; self.buffer.cursor_col = self.buffer.lines[r].len(); }
            (KeyCode::Tab, _)                           => { for _ in 0..4 { self.buffer.insert_char(' '); } }
            (KeyCode::Char(c), _)                       => self.buffer.insert_char(c),
            _ => {}
        }
        false
    }

    fn handle_explorer(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc   => { self.mode = Mode::Normal; self.status_msg = "NORMAL".into(); }
            KeyCode::Up    => { if self.explorer.selected > 0 { self.explorer.selected -= 1; } }
            KeyCode::Down  => { if self.explorer.selected + 1 < self.explorer.entries.len() { self.explorer.selected += 1; } }
            KeyCode::Enter => {
                if let Some(path) = self.explorer.enter() {
                    self.status_msg = match Buffer::from_file(path.clone()) {
                        Ok(buf) => { self.buffer = buf; format!("Opened: {}", path.display()) }
                        Err(e)  => format!("Error: {}", e),
                    };
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Backspace => self.explorer.go_up(),
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
            _ => {}
        }
        false
    }

    fn handle_command(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc   => { self.mode = Mode::Normal; self.command_buf.clear(); self.status_msg = "NORMAL".into(); }
            KeyCode::Enter => { let cmd = self.command_buf.trim().to_string(); self.command_buf.clear(); self.mode = Mode::Normal; return self.exec_command(&cmd); }
            KeyCode::Backspace => { self.command_buf.pop(); }
            KeyCode::Char(c)   => { self.command_buf.push(c); }
            _ => {}
        }
        false
    }

    fn exec_command(&mut self, cmd: &str) -> bool {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        match parts[0] {
            "q"  => { if self.buffer.dirty { self.status_msg = "Unsaved changes! Use :q! or :wq".into(); return false; } return true; }
            "q!" => return true,
            "w"  => { self.status_msg = match self.buffer.save() { Ok(_) => "Saved.".into(), Err(e) => format!("Error: {}", e) }; }
            "wq" => { match self.buffer.save() { Ok(_) => return true, Err(e) => self.status_msg = format!("Error: {}", e) } }
            "new" => { self.buffer = Buffer::empty(); self.status_msg = "New buffer.".into(); }
            "open" => {
                if parts.len() > 1 {
                    let path = PathBuf::from(parts[1].trim());
                    self.status_msg = match Buffer::from_file(path.clone()) {
                        Ok(buf) => { self.buffer = buf; format!("Opened: {}", path.display()) }
                        Err(e) => format!("Error: {}", e),
                    };
                } else { self.status_msg = "Usage: :open <path>".into(); }
            }
            "saveas" => {
                if parts.len() > 1 {
                    let path = PathBuf::from(parts[1].trim());
                    self.status_msg = match self.buffer.save_as(path.clone()) {
                        Ok(_) => format!("Saved as: {}", path.display()),
                        Err(e) => format!("Error: {}", e),
                    };
                } else { self.status_msg = "Usage: :saveas <path>".into(); }
            }
            "explorer" => {
                self.show_explorer = !self.show_explorer;
                self.status_msg = if self.show_explorer { "Explorer shown.".into() } else { "Explorer hidden.".into() };
            }
            _ => { self.status_msg = format!("Unknown command: {}", cmd); }
        }
        false
    }
}

fn pad_str(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width { s.chars().take(width).collect() }
    else { format!("{}{}", s, " ".repeat(width - n)) }
}

fn main() -> io::Result<()> {
    let initial_path = env::args().nth(1).map(PathBuf::from);
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Show)?;
    let mut app = App::new(initial_path)?;
    let mut quit = false;
    while !quit {
        if app.mode == Mode::Help {
            app.render_help(&mut stdout)?;
            continue;
        }
        app.render(&mut stdout)?;
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key)        => { quit = app.handle_key(key); }
                Event::Resize(w, h)    => { app.term_w = w; app.term_h = h; }
                _ => {}
            }
        }
    }
    execute!(stdout, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    println!("Thanks for using AMEK. Bye!");
    Ok(())
}
