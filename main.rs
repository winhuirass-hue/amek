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

// ══════════════════════════════════════════════════════════════════════════════
//  SYNTAX HIGHLIGHTING  (Rust · C · C++ · HTML · CSS · JS · Python · Lua)
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, PartialEq)]
enum TK { Kw, Str, Cmt, Num, Ty, Mac, Tag, Attr, Sel, Prop, Sym, Pun }

struct Tok { text: String, kind: TK }

// VS-Code-dark inspired palette
fn tok_color(k: &TK) -> Color {
    match k {
        TK::Kw   => Color::Rgb { r: 86,  g: 156, b: 214 }, // blue
        TK::Str  => Color::Rgb { r: 206, g: 145, b: 120 }, // orange
        TK::Cmt  => Color::Rgb { r: 106, g: 153, b: 85  }, // green
        TK::Num  => Color::Rgb { r: 181, g: 206, b: 168 }, // light green
        TK::Ty   => Color::Rgb { r: 78,  g: 201, b: 176 }, // teal
        TK::Mac  => Color::Rgb { r: 220, g: 220, b: 170 }, // yellow
        TK::Tag  => Color::Rgb { r: 86,  g: 156, b: 214 }, // blue (HTML tag)
        TK::Attr => Color::Rgb { r: 156, g: 220, b: 254 }, // light blue (attr)
        TK::Sel  => Color::Rgb { r: 215, g: 186, b: 125 }, // gold (CSS selector)
        TK::Prop => Color::Rgb { r: 156, g: 220, b: 254 }, // light blue (CSS prop)
        TK::Sym  => Color::Rgb { r: 212, g: 212, b: 212 }, // white
        TK::Pun  => Color::Rgb { r: 170, g: 170, b: 170 }, // grey
    }
}

fn highlight(line: &str, ext: &str) -> Vec<Tok> {
    match ext {
        "rs"  => lex_generic(line,
            &["fn","let","mut","pub","use","mod","struct","enum","impl","trait",
              "if","else","match","for","while","loop","return","self","super",
              "crate","in","as","where","type","const","static","async","await",
              "move","ref","dyn","true","false","unsafe","extern","break","continue"],
            &["i8","i16","i32","i64","i128","isize","u8","u16","u32","u64","u128",
              "usize","f32","f64","bool","char","str","String","Vec","Option",
              "Result","Box","Arc","Rc","HashMap","HashSet","BTreeMap","Mutex"]),
        "c"|"h" => lex_generic(line,
            &["int","char","float","double","long","short","void","unsigned","signed",
              "if","else","for","while","do","return","struct","enum","typedef","union",
              "switch","case","break","continue","static","extern","const","sizeof",
              "volatile","register","goto","default","auto","inline","restrict"],
            &["size_t","ptrdiff_t","uint8_t","uint16_t","uint32_t","uint64_t",
              "int8_t","int16_t","int32_t","int64_t","bool","FILE","NULL","EOF",
              "stdin","stdout","stderr"]),
        "cpp"|"cc"|"cxx"|"hpp" => lex_generic(line,
            &["class","namespace","template","typename","virtual","override","final",
              "public","private","protected","new","delete","this","operator","friend",
              "using","try","catch","throw","nullptr","true","false","const","static",
              "inline","explicit","mutable","volatile","auto","decltype","constexpr",
              "if","else","for","while","do","return","struct","enum","typedef","union",
              "switch","case","break","continue","sizeof","noexcept","static_assert"],
            &["int","char","float","double","long","short","void","unsigned","signed",
              "bool","size_t","string","vector","map","set","pair","tuple","unique_ptr",
              "shared_ptr","weak_ptr","optional","variant","array","list","deque"]),
        "html"|"htm" => lex_html(line),
        "css"        => lex_css(line),
        "js"|"ts"|"jsx"|"tsx" => lex_generic(line,
            &["const","let","var","function","return","if","else","for","while","do",
              "class","extends","new","this","super","import","export","default",
              "async","await","typeof","instanceof","in","of","switch","case","break",
              "continue","throw","try","catch","finally","delete","void","yield",
              "true","false","null","undefined","static","get","set","from","as"],
            &["Number","String","Boolean","Array","Object","Promise","Map","Set",
              "WeakMap","WeakSet","Symbol","BigInt","Error","Date","Math","JSON",
              "console","window","document","navigator","localStorage","fetch",
              "RegExp","Proxy","Reflect","Iterator","Generator"]),
        "py"  => lex_generic(line,
            &["def","class","if","elif","else","for","while","return","import","from",
              "as","with","try","except","finally","raise","pass","break","continue",
              "lambda","yield","and","or","not","in","is","True","False","None",
              "global","nonlocal","assert","del","print","async","await"],
            &["int","float","str","bool","list","dict","tuple","set","bytes","bytearray",
              "type","object","Exception","BaseException","ValueError","TypeError",
              "RuntimeError","StopIteration","Any","Optional","Union","List","Dict",
              "Tuple","Set","Callable","Iterator","Generator","Awaitable","Coroutine"]),
        "lua" => lex_generic(line,
            &["and","break","do","else","elseif","end","false","for","function","goto",
              "if","in","local","nil","not","or","repeat","return","then","true",
              "until","while"],
            &["string","number","boolean","table","function","thread","userdata",
              "pairs","ipairs","next","select","type","tostring","tonumber","rawget",
              "rawset","rawequal","rawlen","setmetatable","getmetatable","pcall",
              "xpcall","error","assert","load","loadfile","dofile","require",
              "print","math","table","string","io","os","coroutine","package"]),
        _ => vec![Tok { text: line.to_string(), kind: TK::Sym }],
    }
}

fn lex_generic(line: &str, kws: &[&str], tys: &[&str]) -> Vec<Tok> {
    let t = line.trim_start();
    // full-line comment patterns
    if t.starts_with("//") || t.starts_with('#') || t.starts_with("--") {
        return vec![Tok { text: line.to_string(), kind: TK::Cmt }];
    }
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut out = Vec::new();
    while i < len {
        // inline comment
        if i+1 < len && chars[i] == '/' && chars[i+1] == '/' {
            let rest: String = chars[i..].iter().collect();
            out.push(Tok { text: rest, kind: TK::Cmt }); break;
        }
        if i+1 < len && chars[i] == '-' && chars[i+1] == '-' {
            let rest: String = chars[i..].iter().collect();
            out.push(Tok { text: rest, kind: TK::Cmt }); break;
        }
        if chars[i] == '"' || chars[i] == '\'' || chars[i] == '`' {
            let q = chars[i];
            let mut s = String::from(q); i += 1;
            while i < len {
                s.push(chars[i]);
                if chars[i] == '\\' && i+1 < len { i += 1; s.push(chars[i]); }
                else if chars[i] == q { i += 1; break; }
                i += 1;
            }
            out.push(Tok { text: s, kind: TK::Str });
        } else if chars[i].is_ascii_digit() {
            let mut s = String::new();
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_' || chars[i] == 'x' || chars[i] == 'b') {
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
        } else if "{}()[];,.<>|&*+-=!~^%@/\\:?".contains(chars[i]) {
            out.push(Tok { text: chars[i].to_string(), kind: TK::Pun }); i += 1;
        } else {
            out.push(Tok { text: chars[i].to_string(), kind: TK::Sym }); i += 1;
        }
    }
    out
}

fn lex_html(line: &str) -> Vec<Tok> {
    let t = line.trim_start();
    if t.starts_with("<!--") {
        return vec![Tok { text: line.to_string(), kind: TK::Cmt }];
    }
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut out = Vec::new();
    while i < len {
        if chars[i] == '<' {
            // tag start
            let mut tag = String::from('<'); i += 1;
            let closing = i < len && chars[i] == '/';
            if closing { tag.push('/'); i += 1; }
            // tag name
            let mut name = String::new();
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '-') {
                name.push(chars[i]); i += 1;
            }
            out.push(Tok { text: tag, kind: TK::Pun });
            out.push(Tok { text: name, kind: TK::Tag });
            // attributes until >
            while i < len && chars[i] != '>' {
                if chars[i].is_whitespace() {
                    let mut sp = String::new();
                    while i < len && chars[i].is_whitespace() { sp.push(chars[i]); i += 1; }
                    out.push(Tok { text: sp, kind: TK::Sym });
                } else if chars[i] == '"' || chars[i] == '\'' {
                    let q = chars[i];
                    let mut s = String::from(q); i += 1;
                    while i < len { s.push(chars[i]); if chars[i] == q { i += 1; break; } i += 1; }
                    out.push(Tok { text: s, kind: TK::Str });
                } else if chars[i] == '=' {
                    out.push(Tok { text: "=".into(), kind: TK::Pun }); i += 1;
                } else if chars[i] == '/' {
                    out.push(Tok { text: "/".into(), kind: TK::Pun }); i += 1;
                } else {
                    let mut attr = String::new();
                    while i < len && !chars[i].is_whitespace() && chars[i] != '=' && chars[i] != '>' && chars[i] != '/' {
                        attr.push(chars[i]); i += 1;
                    }
                    out.push(Tok { text: attr, kind: TK::Attr });
                }
            }
            if i < len && chars[i] == '>' { out.push(Tok { text: ">".into(), kind: TK::Pun }); i += 1; }
        } else if chars[i] == '&' {
            let mut ent = String::new();
            while i < len && chars[i] != ';' && !chars[i].is_whitespace() { ent.push(chars[i]); i += 1; }
            if i < len && chars[i] == ';' { ent.push(';'); i += 1; }
            out.push(Tok { text: ent, kind: TK::Num });
        } else {
            let mut text = String::new();
            while i < len && chars[i] != '<' && chars[i] != '&' { text.push(chars[i]); i += 1; }
            if !text.is_empty() { out.push(Tok { text, kind: TK::Sym }); }
        }
    }
    out
}

fn lex_css(line: &str) -> Vec<Tok> {
    let t = line.trim_start();
    if t.starts_with("/*") {
        return vec![Tok { text: line.to_string(), kind: TK::Cmt }];
    }
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut out = Vec::new();
    // inside a rule body (has colon → property: value)
    let colon_pos = line.find(':').unwrap_or(usize::MAX);
    let brace_pos = line.find('{').unwrap_or(usize::MAX);
    let in_rule = colon_pos < len && brace_pos == usize::MAX;
    if in_rule {
        // property: value
        let (prop, rest) = line.split_once(':').unwrap_or((line, ""));
        out.push(Tok { text: prop.to_string(), kind: TK::Prop });
        out.push(Tok { text: ":".to_string(), kind: TK::Pun });
        // value tokens
        let vchars: Vec<char> = rest.chars().collect();
        let vlen = vchars.len();
        let mut vi = 0;
        while vi < vlen {
            if vchars[vi] == '"' || vchars[vi] == '\'' {
                let q = vchars[vi];
                let mut s = String::from(q); vi += 1;
                while vi < vlen { s.push(vchars[vi]); if vchars[vi] == q { vi += 1; break; } vi += 1; }
                out.push(Tok { text: s, kind: TK::Str });
            } else if vchars[vi].is_ascii_digit() || (vchars[vi] == '-' && vi+1 < vlen && vchars[vi+1].is_ascii_digit()) {
                let mut s = String::new();
                while vi < vlen && (vchars[vi].is_ascii_alphanumeric() || vchars[vi] == '.' || vchars[vi] == '-' || vchars[vi] == '%') {
                    s.push(vchars[vi]); vi += 1;
                }
                out.push(Tok { text: s, kind: TK::Num });
            } else if vchars[vi] == '#' {
                let mut s = String::from('#');
                vi += 1;
                while vi < vlen && (vchars[vi].is_ascii_alphanumeric()) { s.push(vchars[vi]); vi += 1; }
                out.push(Tok { text: s, kind: TK::Num });
            } else {
                out.push(Tok { text: vchars[vi].to_string(), kind: TK::Sym }); vi += 1;
            }
        }
        return out;
    }
    // selector / at-rule / brace
    while i < len {
        if chars[i] == '{' || chars[i] == '}' || chars[i] == ',' || chars[i] == ';' {
            out.push(Tok { text: chars[i].to_string(), kind: TK::Pun }); i += 1;
        } else if chars[i] == '@' {
            let mut s = String::from('@');
            i += 1;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '-') { s.push(chars[i]); i += 1; }
            out.push(Tok { text: s, kind: TK::Kw });
        } else if chars[i] == '/' && i+1 < len && chars[i+1] == '*' {
            let rest: String = chars[i..].iter().collect();
            out.push(Tok { text: rest, kind: TK::Cmt }); break;
        } else {
            let mut sel = String::new();
            while i < len && chars[i] != '{' && chars[i] != ',' && chars[i] != ';' { sel.push(chars[i]); i += 1; }
            if !sel.is_empty() { out.push(Tok { text: sel, kind: TK::Sel }); }
        }
    }
    out
}

// ══════════════════════════════════════════════════════════════════════════════
//  GIT STATUS
// ══════════════════════════════════════════════════════════════════════════════

struct GitStatus {
    branch: String,
    modified: usize,
    staged: usize,
    untracked: usize,
    ahead: usize,
    behind: usize,
}

impl GitStatus {
    fn load() -> Option<Self> {
        // branch
        let branch_out = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .stdout(Stdio::piped()).stderr(Stdio::null()).output().ok()?;
        if !branch_out.status.success() { return None; }
        let branch = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();
        if branch.is_empty() { return None; }

        // porcelain status
        let status_out = Command::new("git")
            .args(["status", "--porcelain=v1"])
            .stdout(Stdio::piped()).stderr(Stdio::null()).output().ok()?;
        let status_str = String::from_utf8_lossy(&status_out.stdout).to_string();
        let mut modified = 0usize; let mut staged = 0usize; let mut untracked = 0usize;
        for line in status_str.lines() {
            if line.len() < 2 { continue; }
            let x = line.chars().next().unwrap_or(' ');
            let y = line.chars().nth(1).unwrap_or(' ');
            if x == '?' && y == '?' { untracked += 1; continue; }
            if x != ' ' { staged += 1; }
            if y != ' ' { modified += 1; }
        }

        // ahead/behind
        let (mut ahead, mut behind) = (0usize, 0usize);
        if let Ok(ab_out) = Command::new("git")
            .args(["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
            .stdout(Stdio::piped()).stderr(Stdio::null()).output()
        {
            let ab = String::from_utf8_lossy(&ab_out.stdout);
            let parts: Vec<&str> = ab.trim().split('\t').collect();
            if parts.len() == 2 {
                ahead  = parts[0].parse().unwrap_or(0);
                behind = parts[1].parse().unwrap_or(0);
            }
        }

        Some(GitStatus { branch, modified, staged, untracked, ahead, behind })
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  STARTUP / DASHBOARD
// ══════════════════════════════════════════════════════════════════════════════

const LOGO: &[&str] = &[
    r"   █████╗ ███╗   ███╗███████╗██╗  ██╗",
    r"  ██╔══██╗████╗ ████║██╔════╝██║ ██╔╝",
    r"  ███████║██╔████╔██║█████╗  █████╔╝ ",
    r"  ██╔══██║██║╚██╔╝██║██╔══╝  ██╔═██╗ ",
    r"  ██║  ██║██║ ╚═╝ ██║███████╗██║  ██╗",
    r"  ╚═╝  ╚═╝╚═╝     ╚═╝╚══════╝╚═╝  ╚═╝",
];
const TAGLINE: &str = "  A terminal IDE  ·  written in Rust";
const VERSION: &str = "  v0.2.0";

struct Dashboard {
    git: Option<GitStatus>,
    recent: Vec<PathBuf>,    // recent dirs / files from cwd listing
    selected: usize,
    action_sel: usize,       // 0=new 1=open 2=quit
}

impl Dashboard {
    fn new() -> Self {
        let git = GitStatus::load();
        // show top-level files in cwd as quick-open list
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut recent = Vec::new();
        if let Ok(rd) = fs::read_dir(&cwd) {
            let mut files: Vec<PathBuf> = rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            files.sort();
            recent = files.into_iter().take(12).collect();
        }
        Dashboard { git, recent, selected: 0, action_sel: 0 }
    }

    fn render(&self, out: &mut impl Write, tw: u16, th: u16) -> io::Result<()> {
        queue!(out, terminal::Clear(ClearType::All))?;
        let w = tw as usize;
        let h = th as usize;

        // ── logo block ────────────────────────────────────────────────────
        let logo_w = LOGO[0].chars().count();
        let lx = (w.saturating_sub(logo_w)) / 2;
        let logo_lines = LOGO.len();
        let total_h = logo_lines + 2 + 1 + 2 + self.recent.len().min(12) + 6;
        let ly = (h.saturating_sub(total_h)) / 2;

        for (i, ln) in LOGO.iter().enumerate() {
            // gradient: top rows more dim, bottom more bright
            let r_val = (60 + i * 30).min(160) as u8;
            let g_val = (80 + i * 20).min(180) as u8;
            let b_val = (200u8).saturating_sub((i * 10) as u8);
            queue!(out, cursor::MoveTo(lx as u16, (ly + i) as u16),
                style::SetForegroundColor(Color::Rgb { r: r_val, g: g_val, b: b_val }),
                style::Print(ln),
                style::SetForegroundColor(Color::Reset),
            )?;
        }

        // tagline + version
        let tl_x = (w.saturating_sub(TAGLINE.trim().chars().count())) / 2;
        queue!(out, cursor::MoveTo(tl_x as u16, (ly + logo_lines + 1) as u16),
            style::SetForegroundColor(Color::Rgb { r: 120, g: 140, b: 190 }),
            style::Print(TAGLINE.trim()),
        )?;
        let v_x = (w.saturating_sub(VERSION.trim().chars().count())) / 2;
        queue!(out, cursor::MoveTo(v_x as u16, (ly + logo_lines + 2) as u16),
            style::SetForegroundColor(Color::Rgb { r: 70, g: 85, b: 130 }),
            style::Print(VERSION.trim()),
            style::SetForegroundColor(Color::Reset),
        )?;

        // ── git status box ────────────────────────────────────────────────
        let git_y = ly + logo_lines + 4;
        if let Some(ref g) = self.git {
            let box_w = 44usize;
            let bx = (w.saturating_sub(box_w)) / 2;
            self.draw_box(out, bx as u16, git_y as u16, box_w, 5)?;
            // title
            queue!(out, cursor::MoveTo((bx + 2) as u16, git_y as u16),
                style::SetForegroundColor(Color::Rgb { r: 240, g: 200, b: 80 }),
                style::Print("  git "),
                style::SetForegroundColor(Color::Reset),
            )?;
            // branch
            let branch_str = format!("  branch  {}", g.branch);
            queue!(out, cursor::MoveTo((bx + 2) as u16, (git_y + 1) as u16),
                style::SetForegroundColor(Color::Rgb { r: 100, g: 200, b: 120 }),
                style::Print(&branch_str),
                style::SetForegroundColor(Color::Reset),
            )?;
            // stats row
            let stats = format!(
                "  ~{}  +{}  ?{}{}{}",
                g.modified, g.staged, g.untracked,
                if g.ahead  > 0 { format!("  ↑{}", g.ahead)  } else { String::new() },
                if g.behind > 0 { format!("  ↓{}", g.behind) } else { String::new() },
            );
            queue!(out, cursor::MoveTo((bx + 2) as u16, (git_y + 2) as u16),
                style::SetForegroundColor(Color::Rgb { r: 180, g: 180, b: 180 }),
                style::Print(&stats),
                style::SetForegroundColor(Color::Reset),
            )?;
            // legend row
            let legend = "  ~ modified  + staged  ? untracked";
            queue!(out, cursor::MoveTo((bx + 2) as u16, (git_y + 3) as u16),
                style::SetForegroundColor(Color::Rgb { r: 70, g: 80, b: 110 }),
                style::Print(legend),
                style::SetForegroundColor(Color::Reset),
            )?;
        }

        // ── quick-open file list ──────────────────────────────────────────
        let list_y = git_y + if self.git.is_some() { 6 } else { 0 };
        let list_w = 50usize;
        let lx2 = (w.saturating_sub(list_w)) / 2;

        if !self.recent.is_empty() {
            self.draw_box(out, lx2 as u16, list_y as u16, list_w, self.recent.len() + 2)?;
            // title
            queue!(out, cursor::MoveTo((lx2 + 2) as u16, list_y as u16),
                style::SetForegroundColor(Color::Rgb { r: 100, g: 200, b: 240 }),
                style::Print("  files "),
                style::SetForegroundColor(Color::Reset),
            )?;
            for (i, p) in self.recent.iter().enumerate() {
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let ext  = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                let icon = ext_icon(ext);
                let sel = i == self.selected;
                let label = format!(" {} {}", icon, name);
                let label = pad_str(&label, list_w - 4);
                let (bg, fg) = if sel {
                    (Color::Rgb { r: 35, g: 70, b: 140 }, Color::White)
                } else {
                    (Color::Rgb { r: 12, g: 14, b: 22 }, Color::Rgb { r: 170, g: 175, b: 200 })
                };
                queue!(out, cursor::MoveTo((lx2 + 2) as u16, (list_y + 1 + i) as u16),
                    style::SetBackgroundColor(bg), style::SetForegroundColor(fg),
                    style::Print(label),
                    style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
                )?;
            }
        }

        // ── action buttons ────────────────────────────────────────────────
        let btn_y = list_y + self.recent.len() + 3;
        let actions = [" + New File ", " > Open... ", " x  Quit   "];
        let total_btn_w: usize = actions.iter().map(|a| a.chars().count() + 2).sum::<usize>() + actions.len() - 1;
        let mut bx2 = (w.saturating_sub(total_btn_w)) / 2;
        for (i, label) in actions.iter().enumerate() {
            let sel = i == self.action_sel;
            let (bg, fg) = if sel {
                (Color::Rgb { r: 45, g: 90, b: 180 }, Color::White)
            } else {
                (Color::Rgb { r: 25, g: 28, b: 45 }, Color::Rgb { r: 120, g: 135, b: 180 })
            };
            queue!(out, cursor::MoveTo(bx2 as u16, btn_y as u16),
                style::SetBackgroundColor(bg), style::SetForegroundColor(fg),
                style::Print(label),
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
            )?;
            bx2 += label.chars().count() + 2;
        }

        // ── footer hint ───────────────────────────────────────────────────
        let hint = "↑↓ files   ←→ actions   Enter select   n new   q quit";
        let hx = (w.saturating_sub(hint.chars().count())) / 2;
        queue!(out, cursor::MoveTo(hx as u16, th - 1),
            style::SetForegroundColor(Color::Rgb { r: 55, g: 65, b: 100 }),
            style::Print(hint),
            style::SetForegroundColor(Color::Reset),
        )?;

        out.flush()
    }

    fn draw_box(&self, out: &mut impl Write, x: u16, y: u16, w: usize, h: usize) -> io::Result<()> {
        let bc = Color::Rgb { r: 40, g: 50, b: 80 };
        let bg = Color::Rgb { r: 12, g: 14, b: 22 };
        // top
        queue!(out, cursor::MoveTo(x, y),
            style::SetForegroundColor(bc), style::SetBackgroundColor(bg),
            style::Print(format!("╭{}╮", "─".repeat(w - 2))),
            style::SetForegroundColor(Color::Reset), style::SetBackgroundColor(Color::Reset),
        )?;
        // sides + fill
        for r in 1..h {
            queue!(out, cursor::MoveTo(x, y + r as u16),
                style::SetForegroundColor(bc), style::SetBackgroundColor(bg),
                style::Print("│"),
                style::SetForegroundColor(Color::Reset),
                style::Print(pad_str("", w - 2)),
                style::SetForegroundColor(bc),
                style::Print("│"),
                style::SetForegroundColor(Color::Reset), style::SetBackgroundColor(Color::Reset),
            )?;
        }
        // bottom
        queue!(out, cursor::MoveTo(x, y + h as u16),
            style::SetForegroundColor(bc), style::SetBackgroundColor(bg),
            style::Print(format!("╰{}╯", "─".repeat(w - 2))),
            style::SetForegroundColor(Color::Reset), style::SetBackgroundColor(Color::Reset),
        )?;
        Ok(())
    }
}

fn ext_icon(ext: &str) -> &'static str {
    match ext {
        "rs"  => "rs",  "py"  => "py",  "js"  => "js",  "ts"  => "ts",
        "jsx" => "jsx", "tsx" => "tsx", "c"   => "c ",  "cpp" => "c+",
        "h"   => "h ",  "hpp" => "h+",  "html"=> "ht", "css" => "cs",
        "lua" => "lu",  "md"  => "md",  "toml"=> "tm", "json"=> "jn",
        "sh"  => "sh",  "yml"|"yaml" => "ym", "txt" => "tx",
        _ => "  ",
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  MODE
// ══════════════════════════════════════════════════════════════════════════════

#[derive(PartialEq, Clone, Debug)]
enum Mode { Dashboard, Normal, Insert, Visual, Explorer, Command, Help, Terminal }

// ══════════════════════════════════════════════════════════════════════════════
//  SELECTION
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Clone)]
struct Sel { anchor_row: usize, anchor_col: usize }

fn order_sel(ar: usize, ac: usize, cr: usize, cc: usize) -> (usize, usize, usize, usize) {
    if (ar, ac) <= (cr, cc) { (ar, ac, cr, cc) } else { (cr, cc, ar, ac) }
}

// ══════════════════════════════════════════════════════════════════════════════
//  EXPLORER
// ══════════════════════════════════════════════════════════════════════════════

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

// ══════════════════════════════════════════════════════════════════════════════
//  BUFFER
// ══════════════════════════════════════════════════════════════════════════════

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
        Ok(Buffer { lines, path: Some(path), dirty: false, row: 0, col: 0, srow: 0, scol: 0 })
    }
    fn save(&mut self) -> io::Result<()> {
        if let Some(ref p) = self.path { fs::write(p, self.lines.join("\n"))?; self.dirty = false; }
        Ok(())
    }
    fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
        self.path = Some(path); self.save()
    }
    fn c2b(line: &str, ci: usize) -> usize {
        line.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(line.len())
    }
    fn char_count(&self) -> usize { self.lines[self.row].chars().count() }
    fn insert_char(&mut self, c: char) {
        if self.row >= self.lines.len() { self.lines.push(String::new()); }
        let cc = self.lines[self.row].chars().count();
        let col = self.col.min(cc);
        let bi = Self::c2b(&self.lines[self.row], col);
        self.lines[self.row].insert(bi, c);
        self.col += 1; self.dirty = true;
    }
    fn insert_newline(&mut self) {
        let cc = self.lines[self.row].chars().count();
        let col = self.col.min(cc);
        let bi = Self::c2b(&self.lines[self.row], col);
        let rest = self.lines[self.row].split_off(bi);
        self.lines.insert(self.row + 1, rest);
        self.row += 1; self.col = 0; self.dirty = true;
    }
    fn backspace(&mut self) {
        if self.col > 0 {
            let bi = Self::c2b(&self.lines[self.row], self.col - 1);
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
            let bi = Self::c2b(&self.lines[self.row], self.col);
            self.lines[self.row].remove(bi); self.dirty = true;
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next); self.dirty = true;
        }
    }
    fn move_cursor(&mut self, dr: i32, dc: i32) {
        self.row = (self.row as i32 + dr).clamp(0, self.lines.len() as i32 - 1) as usize;
        let cc = self.lines[self.row].chars().count() as i32;
        if dc != 0 { self.col = (self.col as i32 + dc).clamp(0, cc) as usize; }
        else { self.col = self.col.min(cc as usize); }
    }
    fn ext(&self) -> String {
        self.path.as_ref()
            .and_then(|p| p.extension()).and_then(|e| e.to_str())
            .unwrap_or("").to_lowercase()
    }
    fn delete_selection(&mut self, sel: &Sel) -> String {
        let (sr, sc, er, ec) = order_sel(sel.anchor_row, sel.anchor_col, self.row, self.col);
        let deleted;
        if sr == er {
            let sbi = Self::c2b(&self.lines[sr], sc);
            let ebi = Self::c2b(&self.lines[sr], ec);
            deleted = self.lines[sr][sbi..ebi].to_string();
            self.lines[sr].replace_range(sbi..ebi, "");
        } else {
            let sbi = Self::c2b(&self.lines[sr], sc);
            let ebi = Self::c2b(&self.lines[er], ec);
            let tail = self.lines[er][ebi..].to_string();
            let mut d = self.lines[sr][sbi..].to_string();
            for _ in sr+1..=er { d.push('\n'); self.lines.remove(sr + 1); }
            self.lines[sr].truncate(sbi);
            self.lines[sr].push_str(&tail);
            deleted = d;
        }
        self.row = sr; self.col = sc; self.dirty = true;
        deleted
    }
    fn selected_text(&self, sel: &Sel) -> String {
        let (sr, sc, er, ec) = order_sel(sel.anchor_row, sel.anchor_col, self.row, self.col);
        if sr == er {
            let sbi = Self::c2b(&self.lines[sr], sc);
            let ebi = Self::c2b(&self.lines[sr], ec);
            return self.lines[sr][sbi..ebi].to_string();
        }
        let mut out = String::new();
        let sbi = Self::c2b(&self.lines[sr], sc);
        out.push_str(&self.lines[sr][sbi..]);
        for r in sr+1..er { out.push('\n'); out.push_str(&self.lines[r]); }
        let ebi = Self::c2b(&self.lines[er], ec);
        out.push('\n'); out.push_str(&self.lines[er][..ebi]);
        out
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  TERMINAL PANE
// ══════════════════════════════════════════════════════════════════════════════

struct TermPane { lines: Vec<String>, input: String, scroll: usize }
impl TermPane {
    fn new() -> Self { TermPane { lines: vec![], input: String::new(), scroll: 0 } }
    fn run_command(&mut self, cmd: &str) {
        let prompt = format!("❯ {}", cmd);
        self.lines.push(prompt);
        if cmd.trim() == "clear" { self.lines.clear(); self.input.clear(); return; }
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() { self.input.clear(); return; }
        if parts[0] == "cd" {
            let target = parts.get(1).map(|s| PathBuf::from(s))
                .unwrap_or_else(|| env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(".")));
            match env::set_current_dir(&target) {
                Ok(_)  => self.lines.push(format!("  → {}", target.display())),
                Err(e) => self.lines.push(format!("  error: {}", e)),
            }
            self.input.clear(); return;
        }
        match Command::new("sh").arg("-c").arg(cmd).stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
            Ok(o) => {
                for ln in String::from_utf8_lossy(&o.stdout).lines() { self.lines.push(format!("  {}", ln)); }
                for ln in String::from_utf8_lossy(&o.stderr).lines()  { self.lines.push(format!("  ✗ {}", ln)); }
            }
            Err(e) => { self.lines.push(format!("  error: {}", e)); }
        }
        self.input.clear();
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  HELP  (unicode-bordered, syntax-highlighted code samples)
// ══════════════════════════════════════════════════════════════════════════════

struct HelpSection { title: &'static str, entries: &'static [(&'static str, &'static str)] }

const HELP: &[HelpSection] = &[
    HelpSection { title: "NORMAL MODE", entries: &[
        ("i",        "Enter Insert mode"),
        ("v",        "Enter Visual mode"),
        ("e / Tab",  "Focus File Explorer"),
        ("c",        "Open Terminal panel"),
        (":",        "Enter Command mode"),
        ("?",        "This help screen"),
        ("Arrows",   "Move cursor"),
        ("Home/End", "Start / end of line"),
        ("PgUp/Dn",  "Page up / page down"),
        ("Ctrl+S",   "Save file"),
        ("Ctrl+Q",   "Quit AMEK"),
    ]},
    HelpSection { title: "INSERT MODE", entries: &[
        ("Esc",       "Return to Normal mode"),
        ("Enter",     "Insert new line"),
        ("Backspace", "Delete previous char"),
        ("Delete",    "Delete next char"),
        ("Tab",       "Insert 4 spaces"),
        ("Ctrl+S",    "Save file"),
    ]},
    HelpSection { title: "VISUAL MODE", entries: &[
        ("Arrows",  "Extend selection"),
        ("d",       "Delete selection"),
        ("y",       "Yank (copy) selection"),
        ("i",       "Delete sel + enter Insert"),
        ("Esc",     "Cancel selection"),
    ]},
    HelpSection { title: "EXPLORER", entries: &[
        ("Up/Down",   "Navigate entries"),
        ("Enter",     "Open file / enter dir"),
        ("Backspace", "Go up one directory"),
        ("Esc",       "Back to editor"),
    ]},
    HelpSection { title: "TERMINAL", entries: &[
        ("Type",      "Enter shell command"),
        ("Enter",     "Execute command"),
        ("Backspace", "Delete input char"),
        ("cd <dir>",  "Change directory"),
        ("clear",     "Clear terminal output"),
        ("Esc",       "Back to editor"),
    ]},
    HelpSection { title: "COMMANDS", entries: &[
        (":w",          "Save"),
        (":q",          "Quit (warns if dirty)"),
        (":wq",         "Save and quit"),
        (":q!",         "Force quit"),
        (":new",        "New empty buffer"),
        (":open <f>",   "Open file by path"),
        (":saveas <f>", "Save to new path"),
        (":explorer",   "Toggle Explorer panel"),
        (":term",       "Toggle Terminal panel"),
    ]},
    HelpSection { title: "SYNTAX HIGHLIGHT", entries: &[
        ("Rust",    ".rs"),
        ("C",       ".c .h"),
        ("C++",     ".cpp .cc .cxx .hpp"),
        ("HTML",    ".html .htm"),
        ("CSS",     ".css"),
        ("JS/TS",   ".js .ts .jsx .tsx"),
        ("Python",  ".py"),
        ("Lua",     ".lua"),
    ]},
];

struct HelpState { section: usize }
impl HelpState {
    fn new() -> Self { HelpState { section: 0 } }
    fn next(&mut self) { if self.section + 1 < HELP.len() { self.section += 1; } }
    fn prev(&mut self) { if self.section > 0 { self.section -= 1; } }
}

// ══════════════════════════════════════════════════════════════════════════════
//  APP
// ══════════════════════════════════════════════════════════════════════════════

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
    help: HelpState,
    sel: Option<Sel>,
    clipboard: String,
    dash: Dashboard,
}

impl App {
    fn new(path: Option<PathBuf>) -> io::Result<Self> {
        let (tw, th) = terminal::size()?;
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (buffer, start_mode) = if let Some(p) = &path {
            (Buffer::from_file(p.clone()).unwrap_or_else(|_| Buffer::empty()), Mode::Normal)
        } else {
            (Buffer::empty(), Mode::Dashboard)
        };
        Ok(App {
            mode: start_mode, prev_mode: Mode::Normal, buffer,
            explorer: Explorer::new(cwd),
            status: "AMEK  |  i=insert  v=visual  c=terminal  e=explorer  ?=help".into(),
            cmd_buf: String::new(), tw, th,
            show_exp: true, show_term: false,
            term_pane: TermPane::new(),
            help: HelpState::new(),
            sel: None, clipboard: String::new(),
            dash: Dashboard::new(),
        })
    }

    // ── layout ──────────────────────────────────────────────────────────

    fn ex(&self)  -> u16 { if self.show_exp  { EXP_W + 1 } else { 0 } }
    fn eh(&self)  -> u16 { self.th.saturating_sub(2 + if self.show_term { TERM_H } else { 0 }) }
    fn ew(&self)  -> u16 { self.tw.saturating_sub(self.ex()) }

    // ── render dispatcher ────────────────────────────────────────────────

    fn render(&mut self, out: &mut impl Write) -> io::Result<()> {
        match self.mode {
            Mode::Dashboard => self.dash.render(out, self.tw, self.th),
            Mode::Help      => self.render_help(out),
            _               => self.render_editor_frame(out),
        }
    }

    fn render_editor_frame(&mut self, out: &mut impl Write) -> io::Result<()> {
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

    // ── title bar ────────────────────────────────────────────────────────

    fn render_title(&self, out: &mut impl Write) -> io::Result<()> {
        let fname = self.buffer.path.as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "[untitled]".into());
        let dirty = if self.buffer.dirty { " ●" } else { "" };
        let left = format!("  AMEK  │  {}{}", fname, dirty);
        let (badge, badge_bg) = match self.mode {
            Mode::Normal   => ("  NORMAL  ", Color::Rgb { r: 40, g: 70, b: 140 }),
            Mode::Insert   => ("  INSERT  ", Color::Rgb { r: 30, g: 150, b: 70  }),
            Mode::Visual   => ("  VISUAL  ", Color::Rgb { r: 140, g: 60, b: 160 }),
            Mode::Explorer => (" EXPLORER ", Color::Rgb { r: 30, g: 120, b: 170 }),
            Mode::Command  => ("  COMMAND ", Color::Rgb { r: 150, g: 110, b: 20 }),
            Mode::Terminal => (" TERMINAL ", Color::Rgb { r: 20, g: 120, b: 100 }),
            _              => ("  NORMAL  ", Color::Rgb { r: 40, g: 70, b: 140 }),
        };
        let right = format!("{} Ln {}  Col {} ", badge, self.buffer.row+1, self.buffer.col+1);
        let fill  = (self.tw as usize).saturating_sub(left.chars().count() + right.chars().count());
        let bar: String = format!("{}{}{}", left, " ".repeat(fill), right)
            .chars().take(self.tw as usize).collect();
        queue!(out, cursor::MoveTo(0,0),
            style::SetBackgroundColor(Color::Rgb { r: 11, g: 14, b: 30 }),
            style::SetForegroundColor(Color::Rgb { r: 130, g: 155, b: 210 }),
            style::Print(&bar),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        let bx = (self.tw as usize).saturating_sub(right.chars().count()) as u16;
        queue!(out, cursor::MoveTo(bx, 0),
            style::SetBackgroundColor(badge_bg),
            style::SetForegroundColor(Color::Rgb { r: 0, g: 0, b: 0 }),
            style::Print(badge),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )
    }

    // ── explorer ─────────────────────────────────────────────────────────

    fn render_explorer(&mut self, out: &mut impl Write) -> io::Result<()> {
        let h = self.eh();
        let w = EXP_W as usize;
        for row in 0..h {
            queue!(out, cursor::MoveTo(0, row+1),
                style::SetBackgroundColor(Color::Rgb { r: 14, g: 16, b: 24 }),
                style::Print(" ".repeat(w)),
                style::SetBackgroundColor(Color::Reset),
            )?;
        }
        let dir_s = self.explorer.dir.to_string_lossy().into_owned();
        let trimmed = if dir_s.chars().count() > w.saturating_sub(4) {
            let skip = dir_s.chars().count().saturating_sub(w - 4);
            dir_s.chars().skip(skip).collect::<String>()
        } else { dir_s.clone() };
        let header = pad_str(&format!(" ▸ {}", trimmed), w);
        queue!(out, cursor::MoveTo(0,1),
            style::SetBackgroundColor(Color::Rgb { r: 20, g: 24, b: 40 }),
            style::SetForegroundColor(Color::Rgb { r: 80, g: 185, b: 205 }),
            style::Print(header),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        let vis = (h as usize).saturating_sub(2);
        if self.explorer.selected < self.explorer.scroll { self.explorer.scroll = self.explorer.selected; }
        else if self.explorer.selected >= self.explorer.scroll + vis {
            self.explorer.scroll = self.explorer.selected.saturating_sub(vis - 1);
        }
        for (i, entry) in self.explorer.entries.iter().enumerate().skip(self.explorer.scroll).take(vis) {
            let row = (i - self.explorer.scroll + 2) as u16 + 1;
            let is_dir = entry.is_dir();
            let name = entry.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let pfx = if is_dir { "▸ " } else {
                let ext = entry.extension().and_then(|e| e.to_str()).unwrap_or("");
                match ext {
                    "rs" => "◆ ", "py" => "◆ ", "js"|"ts"|"jsx"|"tsx" => "◆ ",
                    "c"|"cpp"|"h"|"hpp" => "◆ ", "html"|"htm" => "◆ ",
                    "css" => "◆ ", "lua" => "◆ ", "md" => "◇ ",
                    _ => "· ",
                }
            };
            let label = pad_str(&format!(" {}{}", pfx, name), w);
            let sel = i == self.explorer.selected;
            let (bg, fg) = if sel {
                (Color::Rgb { r: 30, g: 65, b: 135 }, Color::White)
            } else if is_dir {
                (Color::Rgb { r: 14, g: 16, b: 24 }, Color::Rgb { r: 80, g: 175, b: 205 })
            } else {
                (Color::Rgb { r: 14, g: 16, b: 24 }, Color::Rgb { r: 155, g: 160, b: 180 })
            };
            queue!(out, cursor::MoveTo(0, row),
                style::SetBackgroundColor(bg), style::SetForegroundColor(fg),
                style::Print(label),
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
            )?;
        }
        // separator
        for row in 0..h {
            queue!(out, cursor::MoveTo(EXP_W, row+1),
                style::SetForegroundColor(Color::Rgb { r: 32, g: 40, b: 70 }),
                style::Print("│"),
                style::SetForegroundColor(Color::Reset),
            )?;
        }
        Ok(())
    }

    // ── editor ───────────────────────────────────────────────────────────

    fn render_editor(&mut self, out: &mut impl Write) -> io::Result<()> {
        let ex = self.ex();
        let ew = self.ew() as usize;
        let eh = self.eh() as usize;
        let gutter = 5usize;
        let col_area = ew.saturating_sub(gutter);
        let ext = self.buffer.ext();

        // scroll adjust
        if self.buffer.row < self.buffer.srow { self.buffer.srow = self.buffer.row; }
        else if self.buffer.row >= self.buffer.srow + eh { self.buffer.srow = self.buffer.row - eh + 1; }
        if self.buffer.col < self.buffer.scol { self.buffer.scol = self.buffer.col; }
        else if self.buffer.col >= self.buffer.scol + col_area { self.buffer.scol = self.buffer.col - col_area + 1; }

        let (sel_sr, sel_sc, sel_er, sel_ec) = if let (Some(sel), true) = (&self.sel, self.mode == Mode::Visual) {
            let (a,b,c,d) = order_sel(sel.anchor_row, sel.anchor_col, self.buffer.row, self.buffer.col);
            (a, b, c, d+1)
        } else { (0,0,0,0) };
        let has_sel = self.mode == Mode::Visual && self.sel.is_some();

        for sr in 0..eh {
            let br = sr + self.buffer.srow;
            queue!(out, cursor::MoveTo(ex, sr as u16 + 1))?;
            if br >= self.buffer.lines.len() {
                queue!(out,
                    style::SetForegroundColor(Color::Rgb { r: 34, g: 38, b: 56 }),
                    style::Print(format!("{:>4} ", "~")),
                    style::SetForegroundColor(Color::Reset),
                    style::Print(" ".repeat(col_area)),
                )?; continue;
            }
            let is_cur = br == self.buffer.row;
            let cur_bg = Color::Rgb { r: 17, g: 21, b: 36 };
            if is_cur { queue!(out, style::SetBackgroundColor(cur_bg))?; }
            // gutter
            let gc = if is_cur { Color::Rgb { r: 200, g: 165, b: 45 } }
                     else { Color::Rgb { r: 52, g: 58, b: 75 } };
            queue!(out,
                style::SetForegroundColor(gc),
                style::Print(format!("{:>4} ", br+1)),
                style::SetForegroundColor(Color::Reset),
            )?;
            // syntax tokens
            let line = &self.buffer.lines[br];
            let toks = highlight(line, &ext);
            let sc = self.buffer.scol;
            let mut cp = 0usize;
            for tok in &toks {
                let ts = cp; let te = cp + tok.text.chars().count(); cp = te;
                let ve = sc + col_area;
                if te <= sc || ts >= ve { continue; }
                for (ci, ch) in tok.text.chars().enumerate() {
                    let abs = ts + ci;
                    if abs < sc || abs >= ve { continue; }
                    let in_sel = has_sel && br >= sel_sr && br <= sel_er
                        && abs >= if br == sel_sr { sel_sc } else { 0 }
                        && abs < if br == sel_er { sel_ec } else { usize::MAX };
                    if in_sel {
                        queue!(out,
                            style::SetBackgroundColor(Color::Rgb { r: 55, g: 75, b: 155 }),
                            style::SetForegroundColor(Color::White),
                            style::Print(ch),
                        )?;
                    } else {
                        let bg = if is_cur { cur_bg } else { Color::Reset };
                        queue!(out,
                            style::SetBackgroundColor(bg),
                            style::SetForegroundColor(tok_color(&tok.kind)),
                            style::Print(ch),
                        )?;
                    }
                }
            }
            let vlen = line.chars().skip(sc).take(col_area).count();
            queue!(out,
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
                style::Print(" ".repeat(col_area.saturating_sub(vlen))),
            )?;
        }
        Ok(())
    }

    // ── terminal pane ────────────────────────────────────────────────────

    fn render_term(&mut self, out: &mut impl Write) -> io::Result<()> {
        let top = self.th.saturating_sub(TERM_H + 1);
        let w = self.tw as usize;
        // header bar
        let hdr = pad_str(" ❯ TERMINAL  (Esc = close)", w);
        queue!(out, cursor::MoveTo(0, top),
            style::SetBackgroundColor(Color::Rgb { r: 9, g: 30, b: 26 }),
            style::SetForegroundColor(Color::Rgb { r: 70, g: 200, b: 150 }),
            style::Print(hdr),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        let visible = (TERM_H - 1) as usize;
        let mut all: Vec<String> = self.term_pane.lines.clone();
        all.push(format!("❯ {}▌", self.term_pane.input));
        let start = all.len().saturating_sub(visible);
        for (i, ln) in all.iter().enumerate().skip(start) {
            let row = top + 1 + (i - start) as u16;
            let is_prompt = i == all.len() - 1;
            let (bg, fg) = if is_prompt {
                (Color::Rgb { r: 7, g: 18, b: 16 }, Color::Rgb { r: 70, g: 215, b: 150 })
            } else {
                (Color::Rgb { r: 7, g: 14, b: 12 }, Color::Rgb { r: 165, g: 195, b: 185 })
            };
            queue!(out, cursor::MoveTo(0, row),
                style::SetBackgroundColor(bg), style::SetForegroundColor(fg),
                style::Print(pad_str(ln, w)),
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
            )?;
        }
        Ok(())
    }

    // ── status bar ───────────────────────────────────────────────────────

    fn render_status(&self, out: &mut impl Write) -> io::Result<()> {
        let y = self.th - 1;
        let left = if self.mode == Mode::Visual {
            if let Some(sel) = &self.sel {
                let txt = self.buffer.selected_text(sel);
                format!("  VISUAL  {}ch  {}ln  ", txt.chars().count(), txt.lines().count().max(1))
            } else { format!("  {}  ", self.status) }
        } else { format!("  {}  ", self.status) };
        let ext = self.buffer.ext().to_uppercase();
        let right = format!("  {}  ", if ext.is_empty() { "TXT" } else { &ext });
        let fill = (self.tw as usize).saturating_sub(left.chars().count() + right.chars().count());
        let bar: String = format!("{}{}{}", left, " ".repeat(fill), right)
            .chars().take(self.tw as usize).collect();
        queue!(out, cursor::MoveTo(0, y),
            style::SetBackgroundColor(Color::Rgb { r: 14, g: 20, b: 42 }),
            style::SetForegroundColor(Color::Rgb { r: 120, g: 145, b: 200 }),
            style::Print(bar),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )
    }

    fn render_cmdline(&self, out: &mut impl Write) -> io::Result<()> {
        let y = self.th - 1;
        let content = pad_str(&format!(":{}", self.cmd_buf), self.tw as usize);
        queue!(out, cursor::MoveTo(0, y),
            style::SetBackgroundColor(Color::Rgb { r: 6, g: 7, b: 14 }),
            style::SetForegroundColor(Color::Rgb { r: 210, g: 190, b: 70 }),
            style::Print(content),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )
    }

    // ── help ─────────────────────────────────────────────────────────────

    fn render_help(&mut self, out: &mut impl Write) -> io::Result<()> {
        queue!(out, terminal::Clear(ClearType::All))?;
        let w = self.tw as usize;
        let h = self.th as usize;

        // title bar
        let title = pad_str("  AMEK  Help  ──  ← → switch  Esc close", w);
        queue!(out, cursor::MoveTo(0, 0),
            style::SetBackgroundColor(Color::Rgb { r: 14, g: 18, b: 42 }),
            style::SetForegroundColor(Color::Rgb { r: 90, g: 185, b: 230 }),
            style::Print(title),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;

        // tab strip (row 1)
        let mut tx = 0u16;
        for (i, sec) in HELP.iter().enumerate() {
            let lbl = format!(" {} ", sec.title);
            let (bg, fg) = if i == self.help.section {
                (Color::Rgb { r: 35, g: 110, b: 190 }, Color::White)
            } else {
                (Color::Rgb { r: 18, g: 22, b: 44 }, Color::Rgb { r: 80, g: 105, b: 150 })
            };
            let lw = lbl.chars().count() as u16;
            if tx + lw >= self.tw { break; }
            queue!(out, cursor::MoveTo(tx, 1),
                style::SetBackgroundColor(bg), style::SetForegroundColor(fg),
                style::Print(&lbl),
                style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
            )?;
            tx += lw + 1;
        }

        // separator row 2
        let sep = "─".repeat(w);
        queue!(out, cursor::MoveTo(0, 2),
            style::SetForegroundColor(Color::Rgb { r: 32, g: 42, b: 72 }),
            style::Print(sep),
            style::SetForegroundColor(Color::Reset),
        )?;

        // content box
        if let Some(sec) = HELP.get(self.help.section) {
            let box_w = (w.min(62)).max(30);
            let bx = (w.saturating_sub(box_w)) / 2;
            let by = 3u16;
            // top border
            queue!(out, cursor::MoveTo(bx as u16, by),
                style::SetForegroundColor(Color::Rgb { r: 38, g: 55, b: 100 }),
                style::Print(format!("╭─ {} {}", sec.title, "─".repeat(box_w.saturating_sub(sec.title.chars().count() + 5)))),
                style::Print("╮"),
                style::SetForegroundColor(Color::Reset),
            )?;
            // column headers
            queue!(out, cursor::MoveTo(bx as u16, by + 1),
                style::SetForegroundColor(Color::Rgb { r: 38, g: 55, b: 100 }),
                style::Print("│"),
                style::SetForegroundColor(Color::Rgb { r: 80, g: 100, b: 150 }),
                style::Print(format!("  {:<18} {:<width$}", "Binding", "Description", width = box_w.saturating_sub(22))),
                style::SetForegroundColor(Color::Rgb { r: 38, g: 55, b: 100 }),
                style::Print("│"),
                style::SetForegroundColor(Color::Reset),
            )?;
            // divider
            queue!(out, cursor::MoveTo(bx as u16, by + 2),
                style::SetForegroundColor(Color::Rgb { r: 38, g: 55, b: 100 }),
                style::Print(format!("├{}┤", "─".repeat(box_w - 2))),
                style::SetForegroundColor(Color::Reset),
            )?;
            // entries
            for (ei, (key, desc)) in sec.entries.iter().enumerate() {
                let row = by + 3 + ei as u16;
                if row as usize >= h.saturating_sub(3) { break; }
                queue!(out, cursor::MoveTo(bx as u16, row),
                    style::SetForegroundColor(Color::Rgb { r: 38, g: 55, b: 100 }),
                    style::Print("│"),
                    style::SetForegroundColor(Color::Rgb { r: 200, g: 180, b: 80 }),   // key = gold
                    style::Print(format!("  {:<18}", key)),
                    style::SetForegroundColor(Color::Rgb { r: 185, g: 200, b: 220 }),  // desc = light
                    style::Print(format!("{:<width$}", desc, width = box_w.saturating_sub(22))),
                    style::SetForegroundColor(Color::Rgb { r: 38, g: 55, b: 100 }),
                    style::Print("│"),
                    style::SetForegroundColor(Color::Reset),
                )?;
            }
            // bottom border
            let bot = by + 3 + sec.entries.len() as u16;
            if (bot as usize) < h.saturating_sub(2) {
                queue!(out, cursor::MoveTo(bx as u16, bot),
                    style::SetForegroundColor(Color::Rgb { r: 38, g: 55, b: 100 }),
                    style::Print(format!("╰{}╯", "─".repeat(box_w - 2))),
                    style::SetForegroundColor(Color::Reset),
                )?;
            }
        }

        // footer
        let footer = pad_str("  ← →  switch sections    Esc  close help", w);
        queue!(out, cursor::MoveTo(0, self.th - 1),
            style::SetBackgroundColor(Color::Rgb { r: 12, g: 16, b: 36 }),
            style::SetForegroundColor(Color::Rgb { r: 60, g: 80, b: 130 }),
            style::Print(footer),
            style::SetBackgroundColor(Color::Reset), style::SetForegroundColor(Color::Reset),
        )?;
        out.flush()
    }

    // ── cursor placement ─────────────────────────────────────────────────

    fn place_cursor(&self, out: &mut impl Write) -> io::Result<()> {
        match self.mode {
            Mode::Explorer => {
                let r = (self.explorer.selected.saturating_sub(self.explorer.scroll) + 2) as u16 + 1;
                queue!(out, cursor::MoveTo(1, r))
            }
            Mode::Command => {
                queue!(out, cursor::MoveTo(self.cmd_buf.chars().count() as u16 + 1, self.th - 1))
            }
            Mode::Terminal => {
                // cursor is rendered as block char in prompt, just hide it
                queue!(out, cursor::MoveTo(0, self.th - 1))
            }
            _ => {
                let ex = self.ex();
                let x = ex + 5 + self.buffer.col.saturating_sub(self.buffer.scol) as u16;
                let y = 1 + self.buffer.row.saturating_sub(self.buffer.srow) as u16;
                queue!(out, cursor::MoveTo(x, y))
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    //  INPUT
    // ══════════════════════════════════════════════════════════════════════

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.kind != KeyEventKind::Press { return false; }
        match self.mode {
            Mode::Dashboard => self.on_dashboard(key),
            Mode::Normal    => self.on_normal(key),
            Mode::Insert    => self.on_insert(key),
            Mode::Visual    => self.on_visual(key),
            Mode::Explorer  => self.on_explorer(key),
            Mode::Command   => self.on_command(key),
            Mode::Help      => self.on_help(key),
            Mode::Terminal  => self.on_terminal(key),
        }
    }

    fn on_dashboard(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) | (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Char('n'), _) => {
                self.buffer = Buffer::empty(); self.mode = Mode::Normal;
                self.status = "New buffer.".into();
            }
            (KeyCode::Up, _) => {
                if self.dash.selected > 0 { self.dash.selected -= 1; }
            }
            (KeyCode::Down, _) => {
                if self.dash.selected + 1 < self.dash.recent.len() { self.dash.selected += 1; }
            }
            (KeyCode::Left, _) => {
                if self.dash.action_sel > 0 { self.dash.action_sel -= 1; }
            }
            (KeyCode::Right, _) => {
                if self.dash.action_sel < 2 { self.dash.action_sel += 1; }
            }
            (KeyCode::Enter, _) => {
                match self.dash.action_sel {
                    0 => { self.buffer = Buffer::empty(); self.mode = Mode::Normal; self.status = "New buffer.".into(); }
                    1 => {
                        // open selected file in list
                        if let Some(path) = self.dash.recent.get(self.dash.selected).cloned() {
                            self.status = match Buffer::from_file(path.clone()) {
                                Ok(buf) => { self.buffer = buf; self.mode = Mode::Normal; format!("Opened: {}", path.display()) }
                                Err(e) => format!("Error: {}", e),
                            };
                        }
                    }
                    _ => return true,
                }
            }
            (KeyCode::Char('?'), _) => { self.prev_mode = Mode::Dashboard; self.mode = Mode::Help; }
            _ => {}
        }
        false
    }

    fn on_normal(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.status = match self.buffer.save() { Ok(_) => "Saved.".into(), Err(e) => format!("Error: {}", e) };
            }
            (KeyCode::Char('i'), _) => { self.mode = Mode::Insert; self.status = "-- INSERT --".into(); }
            (KeyCode::Char('v'), _) => {
                self.sel = Some(Sel { anchor_row: self.buffer.row, anchor_col: self.buffer.col });
                self.mode = Mode::Visual;
                self.status = "-- VISUAL --  d=delete  y=yank  Esc=cancel".into();
            }
            (KeyCode::Char('c'), _) => { self.show_term = true; self.mode = Mode::Terminal; self.status = "TERMINAL  |  Esc=back".into(); }
            (KeyCode::Char('e'), _) | (KeyCode::Tab, _) => { self.show_exp = true; self.mode = Mode::Explorer; self.status = "EXPLORER  |  Esc=back".into(); }
            (KeyCode::Char(':'), _) => { self.mode = Mode::Command; self.cmd_buf.clear(); }
            (KeyCode::Char('?'), _) => { self.prev_mode = Mode::Normal; self.mode = Mode::Help; }
            (KeyCode::Up, _)        => self.buffer.move_cursor(-1, 0),
            (KeyCode::Down, _)      => self.buffer.move_cursor(1, 0),
            (KeyCode::Left, _)      => self.buffer.move_cursor(0, -1),
            (KeyCode::Right, _)     => self.buffer.move_cursor(0, 1),
            (KeyCode::Home, _)      => self.buffer.col = 0,
            (KeyCode::End, _)       => { let cc = self.buffer.char_count(); self.buffer.col = cc; }
            (KeyCode::PageUp, _)    => self.buffer.move_cursor(-(self.eh() as i32), 0),
            (KeyCode::PageDown, _)  => self.buffer.move_cursor(self.eh() as i32, 0),
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
            (KeyCode::Enter, _)     => self.buffer.insert_newline(),
            (KeyCode::Backspace, _) => self.buffer.backspace(),
            (KeyCode::Delete, _)    => self.buffer.delete_char(),
            (KeyCode::Up, _)        => self.buffer.move_cursor(-1, 0),
            (KeyCode::Down, _)      => self.buffer.move_cursor(1, 0),
            (KeyCode::Left, _)      => self.buffer.move_cursor(0, -1),
            (KeyCode::Right, _)     => self.buffer.move_cursor(0, 1),
            (KeyCode::Home, _)      => self.buffer.col = 0,
            (KeyCode::End, _)       => { let cc = self.buffer.char_count(); self.buffer.col = cc; }
            (KeyCode::Tab, _)       => { for _ in 0..4 { self.buffer.insert_char(' '); } }
            (KeyCode::Char(c), _)   => self.buffer.insert_char(c),
            _ => {}
        }
        false
    }

    fn on_visual(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => { self.mode = Mode::Normal; self.sel = None; self.status = "NORMAL".into(); }
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
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
                if let Some(sel) = self.sel.take() { self.buffer.delete_selection(&sel); }
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
            KeyCode::Esc        => { self.mode = self.prev_mode.clone(); }
            KeyCode::Left       => self.help.prev(),
            KeyCode::Right      => self.help.next(),
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
            _ => {}
        }
        false
    }

    fn on_terminal(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => { self.mode = Mode::Normal; self.status = "NORMAL".into(); }
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Enter, _) => {
                let cmd = self.term_pane.input.clone();
                self.term_pane.run_command(&cmd);
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

// ══════════════════════════════════════════════════════════════════════════════
//  HELPERS
// ══════════════════════════════════════════════════════════════════════════════

fn pad_str(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w { s.chars().take(w).collect() }
    else { format!("{}{}", s, " ".repeat(w - n)) }
}

// ══════════════════════════════════════════════════════════════════════════════
//  MAIN
// ══════════════════════════════════════════════════════════════════════════════

fn main() -> io::Result<()> {
    let path = env::args().nth(1).map(PathBuf::from);
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let mut app = App::new(path)?;
    let mut quit = false;

    while !quit {
        app.render(&mut stdout)?;
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(k)       => { quit = app.handle_key(k); }
                Event::Resize(w, h) => { app.tw = w; app.th = h; }
                _ => {}
            }
        }
    }

    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;
    println!("Thanks for using AMEK. Bye!");
    Ok(())
}
