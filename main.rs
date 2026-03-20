use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{self, Color},
    terminal::{self, ClearType},
};
use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

// ─── Syntax ──────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum TK { Kw, Str, Cmt, Num, Ty, Mac, Sym, Pun }

struct Tok { text: String, kind: TK }

fn highlight(line: &str, ext: &str) -> Vec<Tok> {
    match ext {
        "rs"  => lex(line,
            &["fn","let","mut","pub","use","mod","struct","enum","impl","trait","if","else",
              "match","for","while","loop","return","self","super","crate","in","as","where",
              "type","const","static","async","await","move","ref","dyn","true","false"],
            &["i8","i16","i32","i64","i128","isize","u8","u16","u32","u64","u128","usize",
              "f32","f64","bool","char","str","String","Vec","Option","Result","Box","Arc","Rc"]),
        "py"  => lex(line,
            &["def","class","if","elif","else","for","while","return","import","from","as",
              "with","try","except","finally","raise","pass","break","continue","lambda",
              "yield","and","or","not","in","is","True","False","None"],
            &["int","float","str","bool","list","dict","tuple","set","bytes","type","object"]),
        "js"|"ts" => lex(line,
            &["const","let","var","function","return","if","else","for","while","class",
              "extends","new","this","import","export","default","async","await","typeof",
              "instanceof","true","false","null","undefined","switch","case","break","of"],
            &["Number","String","Boolean","Array","Object","Promise","Map","Set","Error"]),
        "c"|"cpp"|"h" => lex(line,
            &["int","char","float","double","void","if","else","for","while","do","return",
              "struct","enum","typedef","union","switch","case","break","continue","static",
              "extern","const","sizeof","include","define"],
            &["size_t","uint8_t","uint32_t","int32_t","bool","FILE","NULL"]),
        _ => vec![Tok { text: line.to_string(), kind: TK::Sym }],
    }
}

fn lex(line: &str, kws: &[&str], tys: &[&str]) -> Vec<Tok> {
    let t = line.trim_start();
    if t.starts_with("//") || t.starts_with('#') {
        return vec![Tok { text: line.to_string(), kind: TK::Cmt }];
    }
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut out = Vec::new();
    while i < len {
        if chars[i] == '"' || chars[i] == '\'' {
            let q = chars[i];
            let mut s = String::from(q); i += 1;
            while i < len { s.push(chars[i]); if chars[i] == q { i += 1; break; } i += 1; }
            out.push(Tok { text: s, kind: TK::Str });
        } else if chars[i].is_ascii_digit() {
            let mut s = String::new();
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                s.push(chars[i]); i += 1;
            }
            out.push(Tok { text: s, kind: TK::Num });
        } else if chars[i].is_alphabetic() || chars[i] == '_' {
            let mut w = String::new();
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                w.push(chars[i]); i += 1;
            }
            if i < len && chars[i] == '!' {
                w.push('!'); i += 1;
                out.push(Tok { text: w, kind: TK::Mac });
            } else if kws.contains(&w.as_str()) {
                out.push(Tok { text: w, kind: TK::Kw });
            } else if tys.contains(&w.as_str()) {
                out.push(Tok { text: w, kind: TK::Ty });
            } else {
                out.push(Tok { text: w, kind: TK::Sym });
            }
        } else if "{}()[];,.<>|&*+-=!~^%@".contains(chars[i]) {
            out.push(Tok { text: chars[i].to_string(), kind: TK::Pun }); i += 1;
        } else {
            out.push(Tok { text: chars[i].to_string(), kind: TK::Sym }); i += 1;
        }
    }
    out
}

fn tok_color(k: &TK) -> Color {
    match k {
        TK::Kw  => Color::Rgb { r: 86,  g: 156, b: 214 },
        TK::Str => Color::Rgb { r: 206, g: 145, b: 120 },
        TK::Cmt => Color::Rgb { r: 106, g: 153, b: 85  },
        TK::Num => Color::Rgb { r: 181, g: 206, b: 168 },
        TK::Ty  => Color::Rgb { r: 78,  g: 201, b: 176 },
        TK::Mac => Color::Rgb { r: 220, g: 220, b: 170 },
        TK::Pun => Color::Rgb { r: 180, g: 180, b: 180 },
        TK::Sym => Color::Rgb { r: 212, g: 212, b: 212 },
    }
}

// ─── Mode ────────────────────────────────────────────────────────────────────

#[derive(PartialEq, Clone, Debug)]
enum Mode { Normal, Insert, Visual, Explorer, Command, Help, Terminal }

// ─── Selection ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Sel { anchor_row: usize, anchor_col: usize }

// ─── Explorer ────────────────────────────────────────────────────────────────

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
        if let Some(p) = self.dir.parent() {
            self.dir = p.to_path_buf(); self.selected = 0; self.scroll = 0; self.refresh();
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

// ─── Buffer ───────────────────────────────────────────────────────────────────

struct Buffer {
    lines: Vec<String>,
    path: Option<PathBuf>,
    dirty: bool,
    row: usize, col: usize,
    srow: usize, scol: usize,
}
impl Buffer {
    fn empty() -> Self {
        Buffer { lines: vec![String::new()], path: None, dirty: false,
                 row: 0, col: 0, srow: 0, scol: 0 }
    }
    fn from_file(path: PathBuf) -> io::Result<Self> {
        let content = fs::read_to_string(&path)?;
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else { content.lines().map(|l| l.to_string()).collect() };
        Ok(Buffer { lines, path: Some(path), dirty: false,
                    row: 0, col: 0, srow: 0, scol: 0 })
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
    fn char_to_byte(line: &str, ci: usize) -> usize {
        line.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(line.len())
    }
    fn char_count(&self) -> usize { self.lines[self.row].chars().count() }
    fn insert_char(&mut self, c: char) {
        if self.row >= self.lines.len() { self.lines.push(String::new()); }
        let cc = self.lines[self.row].chars().count();
        let col = self.col.min(cc);
        let bi = Self::char_to_byte(&self.lines[self.row], col);
        self.lines[self.row].insert(bi, c);
        self.col += 1; self.dirty = true;
    }
    fn insert_newline(&mut self) {
        let cc = self.lines[self.row].chars().count();
        let col = self.col.min(cc);
        let bi = Self::char_to_byte(&self.lines[self.row], col);
        let rest = self.lines[self.row].split_off(bi);
        self.lines.insert(self.row + 1, rest);
        self.row += 1; self.col = 0; self.dirty = true;
    }
    fn backspace(&mut self) {
        if self.col > 0 {
            let bi = Self::char_to_byte(&self.lines[self.row], self.col - 1);
            self.lines[self.row].remove(bi);
            self.col -= 1; self.dirty = true;
        } else if self.row > 0 {
            let cur = self.lines.remove(self.row);
            let prev_cc = self.lines[self.row - 1].chars().count();
            self.lines[self.row - 1].push_str(&cur);
            self.row -= 1; self.col = prev_cc; self.dirty = true;
        }
    }
    fn delete_char(&mut self) {
        let cc = self.lines[self.row].chars().count();
        if self.col < cc {
            let bi = Self::char_to_byte(&self.lines[self.row], self.col);
            self.lines[self.row].remove(bi); self.dirty = true;
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next); self.dirty = true;
        }
    }
    fn move_cursor(&mut self, dr: i32, dc: i32) {
        self.row = (self.row as i32 + dr).clamp(0, self.lines.len() as i32 - 1) as usize;
        let cc = self.lines[self.row].chars().count() as i32;
        if dc != 0 {
            self.col = (self.col as i32 + dc).clamp(0, cc) as usize;
        } else {
            self.col = self.col.min(cc as usize);
        }
    }
    fn ext(&self) -> String {
        self.path.as_ref()
            .and_then(|p| p.extension()).and_then(|e| e.to_str())
            .unwrap_or("").to_lowercase()
    }
    // Delete from sel anchor to cursor (returns deleted text)
    fn delete_selection(&mut self, sel: &Sel) -> String {
        let (sr, sc, er, ec) = order_sel(sel.anchor_row, sel.anchor_col, self.row, self.col);
        let mut _deleted = String::new();
        if sr == er {
            let line = &self.lines[sr];
            let sbi = Self::char_to_byte(line, sc);
            let ebi = Self::char_to_byte(line, ec);
            _deleted = self.lines[sr][sbi..ebi].to_string();
            self.lines[sr].replace_range(sbi..ebi, "");
        } else {
            let sbi = Self::char_to_byte(&self.lines[sr], sc);
            let ebi = Self::char_to_byte(&self.lines[er], ec);
            let tail = self.lines[er][ebi..].to_string();
            _deleted = self.lines[sr][sbi..].to_string();
            for _ in sr+1..=er { _deleted.push('\n'); self.lines.remove(sr + 1); }
            self.lines[sr].truncate(sbi);
            self.lines[sr].push_str(&tail);
        }
        self.row = sr; self.col = sc; self.dirty = true;
_deleted
    }
    fn selected_text(&self, sel: &Sel) -> String {
        let (sr, sc, er, ec) = order_sel(sel.anchor_row, sel.anchor_col, self.row, self.col);
        if sr == er {
            let line = &self.lines[sr];
            let sbi = Self::char_to_byte(line, sc);
            let ebi = Self::char_to_byte(line, ec);
            return self.lines[sr][sbi..ebi].to_string();
        }
        let mut out = String::new();
        let sbi = Self::char_to_byte(&self.lines[sr], sc);
        out.push_str(&self.lines[sr][sbi..]);
        for r in sr+1..er { out.push('\n'); out.push_str(&self.lines[r]); }
        let ebi = Self::char_to_byte(&self.lines[er], ec);
        out.push('\n'); out.push_str(&self.lines[er][..ebi]);
        out
    }
}

fn order_sel(ar: usize, ac: usize, cr: usize, cc: usize) -> (usize,usize,usize,usize) {
    if (ar, ac) <= (cr, cc) { (ar, ac, cr, cc) } else { (cr, cc, ar, ac) }
}

// ─── Terminal output lines ────────────────────────────────────────────────────

struct TermPane {
    lines: Vec<String>,
    input: String,
    scroll: usize,
}
impl TermPane {
    fn new() -> Self { TermPane { lines: vec!["$ ".into()], input: String::new(), scroll: 0 } }
    fn run_command(&mut self, cmd: &str) {
        self.lines.push(format!("$ {}", cmd));
        if cmd.trim() == "clear" {
            self.lines.clear();
            self.lines.push("$ ".into());
            self.input.clear();
            return;
        }
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() { self.lines.push(String::new()); self.input.clear(); return; }
        // Handle `cd` ourselves since it can't affect our process from a subprocess
        if parts[0] == "cd" {
            let target = parts.get(1).map(|s| PathBuf::from(s))
                .unwrap_or_else(|| dirs_home());
            match std::env::set_current_dir(&target) {
                Ok(_)  => self.lines.push(format!("  => {}", target.display())),
                Err(e) => self.lines.push(format!("  cd: {}", e)),
            }
            self.input.clear();
            return;
        }
        match Command::new("sh").arg("-c").arg(cmd)
            .stdout(Stdio::piped()).stderr(Stdio::piped()).output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                for ln in stdout.lines() { self.lines.push(format!("  {}", ln)); }
                for ln in stderr.lines() { self.lines.push(format!("  \x1b[err\x1b]{}", ln)); }
                if self.lines.is_empty() || (!stdout.is_empty() || !stderr.is_empty()) { }
                else { self.lines.push(String::new()); }
            }
            Err(e) => { self.lines.push(format!("  error: {}", e)); }
        }
        self.input.clear();
    }
    fn prompt_line(&self) -> String { format!("$ {}_", self.input) }
}

fn dirs_home() -> PathBuf {
    env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

// ─── Help ────────────────────────────────────────────────────────────────────

const HELP_SECTIONS: &[(&str, &[&str])] = &[
    ("NORMAL MODE", &[
        "  i          Enter Insert mode",
        "  v          Enter Visual mode",
        "  e / Tab    Focus file explorer",
        "  c          Open terminal panel",
        "  :          Enter command mode",
        "  ?          This help screen",
        "  Arrows     Move cursor",
        "  Ctrl+S     Save file",
        "  Ctrl+Q     Quit",
    ]),
    ("INSERT MODE", &[
        "  Esc        Return to Normal mode",
        "  Enter      New line",
        "  Backspace  Delete previous char",
        "  Delete     Delete next char",
        "  Tab        Insert 4 spaces",
    ]),
    ("VISUAL MODE", &[
        "  Move       Extend selection",
        "  d          Delete selection",
        "  y          Yank (copy) selection",
        "  Esc        Cancel selection",
    ]),
    ("FILE EXPLORER", &[
        "  Up/Down    Navigate entries",
        "  Enter      Open file / enter dir",
        "  Backspace  Go up one directory",
        "  Esc        Back to editor",
    ]),
    ("TERMINAL PANEL  (c to open)", &[
        "  Type       Enter shell command",
        "  Enter      Run command",
        "  Backspace  Delete input char",
        "  Esc        Back to editor",
        "  cd <dir>   Change directory",
        "  clear      Clear terminal output",
    ]),
    ("COMMANDS  (after :)", &[
        "  :w          Save",
        "  :q          Quit (warns if dirty)",
        "  :wq         Save and quit",
        "  :q!         Force quit",
        "  :new        New empty buffer",
        "  :open <f>   Open file by path",
        "  :saveas <f> Save to new path",
        "  :explorer   Toggle explorer",
        "  :term       Open terminal panel",
    ]),
];

struct Help { section: usize }
impl Help {
    fn new() -> Self { Help { section: 0 } }
    fn next(&mut self) { if self.section + 1 < HELP_SECTIONS.len() { self.section += 1; } }
    fn prev(&mut self) { if self.section > 0 { self.section -= 1; } }
}

// ─── App ─────────────────────────────────────────────────────────────────────

const EXP_W: u16 = 28;
const TERM_H: u16 = 12;

struct App {
    mode: Mode,
    prev_mode: Mode,
    buffer: Buffer,
    explorer: Explorer,
    status: String,
    cmd_buf: String,
    tw: u16, th: u16,
    show_exp: bool,
    show_term: bool,
    term_pane: TermPane,
    help: Help,
    sel: Option<Sel>,
    clipboard: String,
}

impl App {
    fn new(path: Option<PathBuf>) -> io::Result<Self> {
        let (tw, th) = terminal::size()?;
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let buffer = if let Some(p) = &path {
            Buffer::from_file(p.clone()).unwrap_or_else(|_| Buffer::empty())
        } else { Buffer::empty() };
        Ok(App {
            mode: Mode::Normal, prev_mode: Mode::Normal, buffer,
            explorer: Explorer::new(cwd),
            status: "AMEK  |  i=insert  v=visual  c=terminal  e=explorer  ?=help".into(),
            cmd_buf: String::new(),
            tw, th,
            show_exp: true, show_term: false,
            term_pane: TermPane::new(),
            help: Help::new(),
            sel: None, clipboard: String::new(),
        })
    }

    // ── layout helpers ──────────────────────────────────────────────────

    fn editor_x(&self) -> u16 { if self.show_exp { EXP_W + 1 } else { 0 } }
    fn editor_h(&self) -> u16 {
        let used = 2 + if self.show_term { TERM_H } else { 0 };
        self.th.saturating_sub(used)
    }
    fn editor_w(&self) -> u16 { self.tw.saturating_sub(self.editor_x()) }

    // ── render ──────────────────────────────────────────────────────────

    fn render(&mut self, out: &mut impl Write) -> io::Result<()> {
        queue!(out, terminal::Clear(ClearType::All))?;
        self.render_title(out)?;
        if self.show_exp  { self.render_explorer(out)?; }
        self.render_editor(out)?;
        if self.show_term { self.render_term(out)?; }
        self.render_status(out)?;
        if self.mode == Mode::Command { self.render_cmdline(out)?; }
        self.place_cursor(out)?;
        out.flush()
    }

    fn render_title(&self, out: &mut impl Write) -> io::Result<()> {
        let fname = self.buffer.path.as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "[untitled]".into());
        let dirty = if self.buffer.dirty { " *" } else { "" };
        let left = format!("  AMEK  |  {}{}", fname, dirty);
        let (badge, badge_bg) = match self.mode {
            Mode::Normal   => (" NORMAL  ", Color::Rgb { r: 40, g: 70, b: 140 }),
            Mode::Insert   => (" INSERT  ", Color::Rgb { r: 30, g: 150, b: 70 }),
            Mode::Visual   => (" VISUAL  ", Color::Rgb { r: 140, g: 60, b: 160 }),
            Mode::Explorer => (" EXPLORER", Color::Rgb { r: 30, g: 120, b: 170 }),
            Mode::Command  => (" COMMAND ", Color::Rgb { r: 150, g: 110, b: 20 }),
            Mode::Help     => (" HELP    ", Color::Rgb { r: 100, g: 60, b: 20 }),
            Mode::Terminal => (" TERMINAL", Color::Rgb { r: 20, g: 120, b: 100 }),
        };
        let right = format!("{} Ln{}  Col{} ", badge, self.buffer.row+1, self.buffer.col+1);
        let fill = (self.tw as usize).saturating_sub(left.len() + right.len());
        let bar: String = format!("{}{}{}", left, " ".repeat(fill), right)
            .chars().take(self.tw as usize).collect();
        queue!(out, cursor::MoveTo(0,0),
            style::SetBackgroundColor(Color::Rgb { r: 13, g: 17, b: 35 }),
            style::SetForegroundColor(Color::Rgb { r: 140, g: 165, b: 215 }),
            style::Print(&bar),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        let bx = (self.tw as usize).saturating_sub(right.len()) as u16;
        queue!(out, cursor::MoveTo(bx, 0),
            style::SetBackgroundColor(badge_bg),
            style::SetForegroundColor(Color::Rgb { r: 0, g: 0, b: 0 }),
            style::Print(badge),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )
    }

    fn render_explorer(&mut self, out: &mut impl Write) -> io::Result<()> {
        let h = self.th.saturating_sub(2 + if self.show_term { TERM_H } else { 0 });
        let w = EXP_W as usize;
        for row in 0..h {
            queue!(out, cursor::MoveTo(0, row+1),
                style::SetBackgroundColor(Color::Rgb { r: 16, g: 18, b: 26 }),
                style::Print(" ".repeat(w)),
                style::SetBackgroundColor(Color::Reset),
            )?;
        }
        let dir_str = self.explorer.dir.to_string_lossy().into_owned();
        let trimmed = if dir_str.len() > w.saturating_sub(3) {
            &dir_str[dir_str.len().saturating_sub(w.saturating_sub(3))..]
        } else { &dir_str };
        let header = pad_str(&format!(" > {}", trimmed), w);
        queue!(out, cursor::MoveTo(0,1),
            style::SetBackgroundColor(Color::Rgb { r: 24, g: 28, b: 44 }),
            style::SetForegroundColor(Color::Rgb { r: 90, g: 190, b: 210 }),
            style::Print(header),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        let vis = (h as usize).saturating_sub(2);
        if self.explorer.selected < self.explorer.scroll { self.explorer.scroll = self.explorer.selected; }
        else if self.explorer.selected >= self.explorer.scroll + vis {
            self.explorer.scroll = self.explorer.selected.saturating_sub(vis - 1);
        }
        for (i, entry) in self.explorer.entries.iter().enumerate()
            .skip(self.explorer.scroll).take(vis)
        {
            let row = (i - self.explorer.scroll + 2) as u16 + 1;
            let is_dir = entry.is_dir();
            let name = entry.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let pfx = if is_dir { "/ " } else { "  " };
            let label = pad_str(&format!(" {}{}", pfx, name), w);
            let sel = i == self.explorer.selected;
            let (bg, fg) = if sel {
                (Color::Rgb { r: 35, g: 75, b: 155 }, Color::White)
            } else if is_dir {
                (Color::Rgb { r: 16, g: 18, b: 26 }, Color::Rgb { r: 90, g: 175, b: 210 })
            } else {
                (Color::Rgb { r: 16, g: 18, b: 26 }, Color::Rgb { r: 165, g: 165, b: 175 })
            };
            queue!(out, cursor::MoveTo(0, row),
                style::SetBackgroundColor(bg), style::SetForegroundColor(fg),
                style::Print(label),
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
            )?;
        }
        for row in 0..h {
            queue!(out, cursor::MoveTo(EXP_W, row+1),
                style::SetForegroundColor(Color::Rgb { r: 35, g: 45, b: 75 }),
                style::Print("|"),
                style::SetForegroundColor(Color::Reset),
            )?;
        }
        Ok(())
    }

    fn render_editor(&mut self, out: &mut impl Write) -> io::Result<()> {
        let ex = self.editor_x();
        let ew = self.editor_w() as usize;
        let eh = self.editor_h() as usize;
        let gutter = 5usize;
        let col_area = ew.saturating_sub(gutter);
        let ext = self.buffer.ext();
        // adjust scroll
        if self.buffer.row < self.buffer.srow { self.buffer.srow = self.buffer.row; }
        else if self.buffer.row >= self.buffer.srow + eh { self.buffer.srow = self.buffer.row - eh + 1; }
        if self.buffer.col < self.buffer.scol { self.buffer.scol = self.buffer.col; }
        else if self.buffer.col >= self.buffer.scol + col_area { self.buffer.scol = self.buffer.col - col_area + 1; }

        let (sel_sr, sel_sc, sel_er, sel_ec) = if let (Some(sel), true) = (&self.sel, self.mode == Mode::Visual) {
            let (a,b,c,d) = order_sel(sel.anchor_row, sel.anchor_col, self.buffer.row, self.buffer.col);
            (a, b, c, d+1) // +1 to include cursor char
        } else { (0,0,0,0) };
        let has_sel = self.mode == Mode::Visual && self.sel.is_some();

        for sr in 0..eh {
            let br = sr + self.buffer.srow;
            queue!(out, cursor::MoveTo(ex, sr as u16 + 1))?;
            if br >= self.buffer.lines.len() {
                queue!(out,
                    style::SetForegroundColor(Color::Rgb { r: 38, g: 42, b: 60 }),
                    style::Print(format!("{:>4} ", "~")),
                    style::SetForegroundColor(Color::Reset),
                    style::Print(" ".repeat(col_area)),
                )?; continue;
            }
            // current line highlight
            let is_cur = br == self.buffer.row;
            let gc = if is_cur { Color::Rgb { r: 210, g: 170, b: 50 } }
                     else      { Color::Rgb { r: 55, g: 60, b: 78 } };
            if is_cur {
                queue!(out, style::SetBackgroundColor(Color::Rgb { r: 18, g: 22, b: 38 }))?;
            }
            queue!(out, style::SetForegroundColor(gc),
                style::Print(format!("{:>4} ", br+1)),
                style::SetForegroundColor(Color::Reset),
            )?;
            // render tokens with optional selection highlight
            let line = &self.buffer.lines[br];
            let toks = highlight(line, &ext);
            let sc = self.buffer.scol;
            let mut cp = 0usize; // char position
            for tok in &toks {
                let ts = cp; let te = cp + tok.text.chars().count(); cp = te;
                let ve = sc + col_area;
                if te <= sc || ts >= ve { continue; }
                // render char by char for selection coloring
                for (ci, ch) in tok.text.chars().enumerate() {
                    let abs = ts + ci;
                    if abs < sc || abs >= ve { continue; }
                    let in_sel = has_sel && br >= sel_sr && br <= sel_er
                        && abs >= if br == sel_sr { sel_sc } else { 0 }
                        && abs < if br == sel_er { sel_ec } else { usize::MAX };
                    if in_sel {
                        queue!(out,
                            style::SetBackgroundColor(Color::Rgb { r: 60, g: 80, b: 160 }),
                            style::SetForegroundColor(Color::White),
                        )?;
                    } else {
                        let bg = if is_cur { Color::Rgb { r: 18, g: 22, b: 38 } } else { Color::Reset };
                        queue!(out,
                            style::SetBackgroundColor(bg),
                            style::SetForegroundColor(tok_color(&tok.kind)),
                        )?;
                    }
                    queue!(out, style::Print(ch))?;
                }
            }
            let visible_len = line.chars().skip(sc).take(col_area).count();
            let pad = col_area.saturating_sub(visible_len);
            queue!(out,
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
                style::Print(" ".repeat(pad)),
            )?;
        }
        Ok(())
    }

    fn render_term(&mut self, out: &mut impl Write) -> io::Result<()> {
        let top = self.th.saturating_sub(TERM_H + 1);
        let w = self.tw as usize;
        // Header bar
        let hdr = pad_str(" TERMINAL  (Esc to close)", w);
        queue!(out, cursor::MoveTo(0, top),
            style::SetBackgroundColor(Color::Rgb { r: 10, g: 35, b: 30 }),
            style::SetForegroundColor(Color::Rgb { r: 80, g: 210, b: 160 }),
            style::Print(hdr),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        // Adjust scroll so prompt is always visible
        let visible = (TERM_H - 1) as usize;
        let total = self.term_pane.lines.len() + 1; // +1 for input prompt
        if total > visible {
            self.term_pane.scroll = total - visible;
        }
        // Output lines
        let all_lines: Vec<String> = {
            let mut v = self.term_pane.lines.clone();
            v.push(self.term_pane.prompt_line());
            v
        };
        for (i, ln) in all_lines.iter().enumerate().skip(self.term_pane.scroll).take(visible) {
            let row = top + 1 + (i - self.term_pane.scroll) as u16;
            let is_prompt = i == all_lines.len() - 1;
            let (bg, fg) = if is_prompt {
                (Color::Rgb { r: 8, g: 20, b: 18 }, Color::Rgb { r: 80, g: 220, b: 160 })
            } else {
                (Color::Rgb { r: 8, g: 16, b: 14 }, Color::Rgb { r: 180, g: 200, b: 195 })
            };
            let label = pad_str(ln, w);
            queue!(out, cursor::MoveTo(0, row),
                style::SetBackgroundColor(bg), style::SetForegroundColor(fg),
                style::Print(label),
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
            )?;
        }
        Ok(())
    }

    fn render_status(&self, out: &mut impl Write) -> io::Result<()> {
        let y = self.th - 1;
        let left = format!("  {}  ", self.status);
        let ext = self.buffer.ext().to_uppercase();
        let right = format!("  {}  ", if ext.is_empty() { "TXT" } else { &ext });
        // Visual: show selection size
        let left = if self.mode == Mode::Visual {
            if let Some(sel) = &self.sel {
                let txt = self.buffer.selected_text(sel);
                let chars = txt.chars().count();
                let lines = txt.lines().count().max(1);
                format!("  VISUAL  {}ch  {}ln  ", chars, lines)
            } else { left }
        } else { left };
        let fill = (self.tw as usize).saturating_sub(left.len() + right.len());
        let bar: String = format!("{}{}{}", left, " ".repeat(fill), right)
            .chars().take(self.tw as usize).collect();
        queue!(out, cursor::MoveTo(0, y),
            style::SetBackgroundColor(Color::Rgb { r: 16, g: 22, b: 44 }),
            style::SetForegroundColor(Color::Rgb { r: 130, g: 155, b: 205 }),
            style::Print(bar),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )
    }

    fn render_cmdline(&self, out: &mut impl Write) -> io::Result<()> {
        let y = self.th - 1;
        let content = format!(":{}", self.cmd_buf);
        let content = pad_str(&content, self.tw as usize);
        queue!(out, cursor::MoveTo(0, y),
            style::SetBackgroundColor(Color::Rgb { r: 7, g: 8, b: 16 }),
            style::SetForegroundColor(Color::Rgb { r: 215, g: 195, b: 75 }),
            style::Print(content),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )
    }

    fn render_help(&mut self, out: &mut impl Write) -> io::Result<()> {
        queue!(out, terminal::Clear(ClearType::All))?;
        let w = self.tw as usize;
        let h = self.th as usize;
        // Title
        let title = pad_str("  AMEK  Help  (← → to switch sections  Esc to close)", w);
        queue!(out, cursor::MoveTo(0,0),
            style::SetBackgroundColor(Color::Rgb { r: 18, g: 22, b: 50 }),
            style::SetForegroundColor(Color::Rgb { r: 100, g: 200, b: 240 }),
            style::Print(title),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        // Section tabs
        let mut tx = 0u16;
        for (i, (name, _)) in HELP_SECTIONS.iter().enumerate() {
            let lbl = format!(" {} ", name);
            let (bg, fg) = if i == self.help.section {
                (Color::Rgb { r: 40, g: 120, b: 200 }, Color::White)
            } else {
                (Color::Rgb { r: 22, g: 26, b: 48 }, Color::Rgb { r: 100, g: 120, b: 160 })
            };
            queue!(out, cursor::MoveTo(tx, 1),
                style::SetBackgroundColor(bg), style::SetForegroundColor(fg),
                style::Print(&lbl),
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
            )?;
            tx += lbl.len() as u16 + 1;
        }
        // Section content
        if let Some((sec_name, lines)) = HELP_SECTIONS.get(self.help.section) {
            let box_w = w.min(60);
            let bx = (w.saturating_sub(box_w)) / 2;
            let by = 3usize;
            // Draw box
            queue!(out, cursor::MoveTo(bx as u16, by as u16),
                style::SetForegroundColor(Color::Rgb { r: 50, g: 80, b: 140 }),
                style::Print(format!("+{}+", "-".repeat(box_w - 2))),
                style::SetForegroundColor(Color::Reset),
            )?;
            let header = format!("| {:^width$} |", sec_name, width = box_w - 4);
            queue!(out, cursor::MoveTo(bx as u16, by as u16 + 1),
                style::SetForegroundColor(Color::Rgb { r: 100, g: 200, b: 240 }),
                style::Print(header),
                style::SetForegroundColor(Color::Reset),
            )?;
            queue!(out, cursor::MoveTo(bx as u16, by as u16 + 2),
                style::SetForegroundColor(Color::Rgb { r: 50, g: 80, b: 140 }),
                style::Print(format!("+{}+", "-".repeat(box_w - 2))),
                style::SetForegroundColor(Color::Reset),
            )?;
            for (li, line) in lines.iter().enumerate() {
                let row = by + 3 + li;
                if row >= h - 2 { break; }
                let body = format!("| {:<width$} |", line, width = box_w - 4);
                queue!(out, cursor::MoveTo(bx as u16, row as u16),
                    style::SetForegroundColor(Color::Rgb { r: 185, g: 200, b: 220 }),
                    style::Print(body),
                    style::SetForegroundColor(Color::Reset),
                )?;
            }
            let bot_row = by + 3 + lines.len();
            if bot_row < h - 1 {
                queue!(out, cursor::MoveTo(bx as u16, bot_row as u16),
                    style::SetForegroundColor(Color::Rgb { r: 50, g: 80, b: 140 }),
                    style::Print(format!("+{}+", "-".repeat(box_w - 2))),
                    style::SetForegroundColor(Color::Reset),
                )?;
            }
        }
        // Footer
        let footer = pad_str("  Left/Right: switch section   Esc: close help", w);
        queue!(out, cursor::MoveTo(0, self.th - 1),
            style::SetBackgroundColor(Color::Rgb { r: 16, g: 20, b: 40 }),
            style::SetForegroundColor(Color::Rgb { r: 90, g: 110, b: 160 }),
            style::Print(footer),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        out.flush()
    }

    fn place_cursor(&self, out: &mut impl Write) -> io::Result<()> {
        match self.mode {
            Mode::Explorer => {
                let r = (self.explorer.selected.saturating_sub(self.explorer.scroll) + 2) as u16 + 1;
                queue!(out, cursor::MoveTo(1, r))
            }
            Mode::Command => {
                queue!(out, cursor::MoveTo(self.cmd_buf.len() as u16 + 1, self.th - 1))
            }
            Mode::Terminal => {
                let y = self.th.saturating_sub(1);
                let x = (2 + self.term_pane.input.chars().count()) as u16;
                queue!(out, cursor::MoveTo(x, y))
            }
            _ => {
                let ex = self.editor_x();
                let x = ex + 5 + self.buffer.col.saturating_sub(self.buffer.scol) as u16;
                let y = 1 + self.buffer.row.saturating_sub(self.buffer.srow) as u16;
                queue!(out, cursor::MoveTo(x, y))
            }
        }
    }

    // ── input ───────────────────────────────────────────────────────────

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // *** THE FIX: ignore Release and Repeat events — only process Press ***
        if key.kind != KeyEventKind::Press { return false; }

        match self.mode {
            Mode::Normal   => self.on_normal(key),
            Mode::Insert   => self.on_insert(key),
            Mode::Visual   => self.on_visual(key),
            Mode::Explorer => self.on_explorer(key),
            Mode::Command  => self.on_command(key),
            Mode::Help     => self.on_help(key),
            Mode::Terminal => self.on_terminal(key),
        }
    }

    fn on_normal(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.status = match self.buffer.save() {
                    Ok(_) => "Saved.".into(), Err(e) => format!("Error: {}", e),
                };
            }
            (KeyCode::Char('i'), _) => {
                self.mode = Mode::Insert; self.sel = None;
                self.status = "-- INSERT --".into();
            }
            (KeyCode::Char('v'), _) => {
                self.sel = Some(Sel { anchor_row: self.buffer.row, anchor_col: self.buffer.col });
                self.mode = Mode::Visual;
                self.status = "-- VISUAL --  d=delete  y=yank  Esc=cancel".into();
            }
            (KeyCode::Char('c'), _) => {
                self.show_term = true; self.mode = Mode::Terminal;
                self.status = "TERMINAL  |  Enter=run  Esc=back".into();
            }
            (KeyCode::Char('e'), _) | (KeyCode::Tab, _) => {
                self.show_exp = true; self.mode = Mode::Explorer;
                self.status = "EXPLORER  |  Enter=open  Bksp=up  Esc=back".into();
            }
            (KeyCode::Char(':'), _) => { self.mode = Mode::Command; self.cmd_buf.clear(); }
            (KeyCode::Char('?'), _) => { self.prev_mode = self.mode.clone(); self.mode = Mode::Help; }
            (KeyCode::Up, _)        => self.buffer.move_cursor(-1, 0),
            (KeyCode::Down, _)      => self.buffer.move_cursor(1, 0),
            (KeyCode::Left, _)      => self.buffer.move_cursor(0, -1),
            (KeyCode::Right, _)     => self.buffer.move_cursor(0, 1),
            (KeyCode::Home, _)      => self.buffer.col = 0,
            (KeyCode::End, _)       => { let cc = self.buffer.char_count(); self.buffer.col = cc; }
            (KeyCode::PageUp, _)    => self.buffer.move_cursor(-(self.editor_h() as i32), 0),
            (KeyCode::PageDown, _)  => self.buffer.move_cursor(self.editor_h() as i32, 0),
            _ => {}
        }
        false
    }

    fn on_insert(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => { self.mode = Mode::Normal; self.status = "NORMAL".into(); }
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.status = match self.buffer.save() { Ok(_) => "Saved.".into(), Err(e) => format!("Error: {}", e) };
            }
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Enter, _)      => self.buffer.insert_newline(),
            (KeyCode::Backspace, _)  => self.buffer.backspace(),
            (KeyCode::Delete, _)     => self.buffer.delete_char(),
            (KeyCode::Up, _)         => self.buffer.move_cursor(-1, 0),
            (KeyCode::Down, _)       => self.buffer.move_cursor(1, 0),
            (KeyCode::Left, _)       => self.buffer.move_cursor(0, -1),
            (KeyCode::Right, _)      => self.buffer.move_cursor(0, 1),
            (KeyCode::Home, _)       => self.buffer.col = 0,
            (KeyCode::End, _)        => { let cc = self.buffer.char_count(); self.buffer.col = cc; }
            (KeyCode::Tab, _)        => { for _ in 0..4 { self.buffer.insert_char(' '); } }
            (KeyCode::Char(c), _)    => self.buffer.insert_char(c),
            _ => {}
        }
        false
    }

    fn on_visual(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => { self.mode = Mode::Normal; self.sel = None; self.status = "NORMAL".into(); }
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
            // move extends selection
            (KeyCode::Up, _)    => self.buffer.move_cursor(-1, 0),
            (KeyCode::Down, _)  => self.buffer.move_cursor(1, 0),
            (KeyCode::Left, _)  => self.buffer.move_cursor(0, -1),
            (KeyCode::Right, _) => self.buffer.move_cursor(0, 1),
            (KeyCode::Home, _)  => self.buffer.col = 0,
            (KeyCode::End, _)   => { let cc = self.buffer.char_count(); self.buffer.col = cc; }
            (KeyCode::Char('d'), _) => {
                if let Some(sel) = self.sel.take() {
                    self.clipboard = self.buffer.delete_selection(&sel);
                    self.mode = Mode::Normal;
                    self.status = format!("Deleted {} chars.", self.clipboard.chars().count());
                }
            }
            (KeyCode::Char('y'), _) => {
                if let Some(ref sel) = self.sel {
                    self.clipboard = self.buffer.selected_text(sel);
                    let n = self.clipboard.chars().count();
                    self.mode = Mode::Normal; self.sel = None;
                    self.status = format!("Yanked {} chars.", n);
                }
            }
            (KeyCode::Char('i'), _) => {
                // enter insert replacing selection
                if let Some(sel) = self.sel.take() {
                    self.buffer.delete_selection(&sel);
                }
                self.mode = Mode::Insert; self.status = "-- INSERT --".into();
            }
            _ => {}
        }
        false
    }

    fn on_explorer(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => { self.mode = Mode::Normal; self.status = "NORMAL".into(); }
            KeyCode::Up   => { if self.explorer.selected > 0 { self.explorer.selected -= 1; } }
            KeyCode::Down => { if self.explorer.selected + 1 < self.explorer.entries.len() { self.explorer.selected += 1; } }
            KeyCode::Enter => {
                if let Some(path) = self.explorer.enter() {
                    self.status = match Buffer::from_file(path.clone()) {
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

    fn on_command(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc   => { self.mode = Mode::Normal; self.cmd_buf.clear(); self.status = "NORMAL".into(); }
            KeyCode::Enter => {
                let cmd = self.cmd_buf.trim().to_string();
                self.cmd_buf.clear(); self.mode = Mode::Normal;
                return self.exec_cmd(&cmd);
            }
            KeyCode::Backspace => { self.cmd_buf.pop(); }
            KeyCode::Char(c)   => { self.cmd_buf.push(c); }
            _ => {}
        }
        false
    }

    fn on_help(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc              => { self.mode = self.prev_mode.clone(); }
            KeyCode::Left             => self.help.prev(),
            KeyCode::Right            => self.help.next(),
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
            _ => {}
        }
        false
    }

    fn on_terminal(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _)  => { self.mode = Mode::Normal; self.status = "NORMAL".into(); }
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Enter, _) => {
                let cmd = self.term_pane.input.clone();
                if !cmd.trim().is_empty() {
                    self.term_pane.run_command(&cmd);
                } else {
                    self.term_pane.lines.push(String::new());
                    self.term_pane.input.clear();
                }
            }
            (KeyCode::Backspace, _) => { self.term_pane.input.pop(); }
            (KeyCode::Char(c), _)   => { self.term_pane.input.push(c); }
            _ => {}
        }
        false
    }

    fn exec_cmd(&mut self, cmd: &str) -> bool {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        match parts[0] {
            "q"  => { if self.buffer.dirty { self.status = "Unsaved! Use :q! or :wq".into(); return false; } return true; }
            "q!" => return true,
            "w"  => { self.status = match self.buffer.save() { Ok(_) => "Saved.".into(), Err(e) => format!("Error: {}", e) }; }
            "wq" => { match self.buffer.save() { Ok(_) => return true, Err(e) => self.status = format!("Error: {}", e) } }
            "new" => { self.buffer = Buffer::empty(); self.status = "New buffer.".into(); }
            "open" => {
                if parts.len() > 1 {
                    let path = PathBuf::from(parts[1].trim());
                    self.status = match Buffer::from_file(path.clone()) {
                        Ok(buf) => { self.buffer = buf; format!("Opened: {}", path.display()) }
                        Err(e) => format!("Error: {}", e),
                    };
                } else { self.status = "Usage: :open <path>".into(); }
            }
            "saveas" => {
                if parts.len() > 1 {
                    let path = PathBuf::from(parts[1].trim());
                    self.status = match self.buffer.save_as(path.clone()) {
                        Ok(_) => format!("Saved as: {}", path.display()),
                        Err(e) => format!("Error: {}", e),
                    };
                } else { self.status = "Usage: :saveas <path>".into(); }
            }
            "explorer" => {
                self.show_exp = !self.show_exp;
                self.status = if self.show_exp { "Explorer shown.".into() } else { "Explorer hidden.".into() };
            }
            "term" => {
                self.show_term = !self.show_term;
                if self.show_term { self.mode = Mode::Terminal; }
                self.status = if self.show_term { "Terminal opened.".into() } else { "Terminal closed.".into() };
            }
            _ => { self.status = format!("Unknown command: {}", cmd); }
        }
        false
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn pad_str(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w { s.chars().take(w).collect() }
    else { format!("{}{}", s, " ".repeat(w - n)) }
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let path = env::args().nth(1).map(PathBuf::from);
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Show)?;

    let mut app = App::new(path)?;
    let mut quit = false;

    while !quit {
        if app.mode == Mode::Help {
            app.render_help(&mut stdout)?;
        } else {
            app.render(&mut stdout)?;
        }
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(k)       => { quit = app.handle_key(k); }
                Event::Resize(w, h) => { app.tw = w; app.th = h; }
                _ => {}
            }
        }
    }

    execute!(stdout, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    println!("Thanks for using AMEK. Bye!");
    Ok(())
}
