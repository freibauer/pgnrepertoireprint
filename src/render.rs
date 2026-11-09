//! Page layout: overview table pages, then two-column footnote pages, in the
//! style of a ChessBase repertoire print. Every move carries its evaluation
//! with one decimal; no other annotation is printed.

use crate::pdf::{Font, PAGE_H, PAGE_W, Page, width};
use crate::pgn::Game;
use crate::repertoire::Repertoire;

const MARGIN: f32 = 36.0;
const TOP: f32 = 36.0;
const BOTTOM: f32 = PAGE_H - 34.0;
const GRAY_EVAL: f32 = 0.35;

// table
const MOVE_SIZE: f32 = 10.0;
const SUP_SIZE: f32 = 6.5;
const EVAL_SIZE: f32 = 7.0;
const ROW_H: f32 = 36.0;
const ROWNUM_W: f32 = 26.0;

// notes
const NOTE_SIZE: f32 = 9.0;
const NOTE_LINE: f32 = 11.6;
const INDENT: f32 = 11.0;
const GUTTER: f32 = 26.0;

pub struct Options {
    pub german: bool,
    pub title: String,
}

/// SAN in the chosen notation (German: S L T D, castling 0-0 like ChessBase).
fn notation(san: &str, german: bool) -> String {
    let s = if san.starts_with("O-O") { san.replace('O', "0") } else { san.to_string() };
    if !german {
        return s;
    }
    s.chars()
        .map(|c| match c {
            'N' => 'S',
            'B' => 'L',
            'R' => 'T',
            'Q' => 'D',
            c => c,
        })
        .collect()
}

struct Ctx<'a> {
    g: &'a Game,
    opt: &'a Options,
    rep: &'a Repertoire,
}

impl Ctx<'_> {
    fn san(&self, n: usize) -> String {
        notation(&self.g.nodes[n].san, self.opt.german)
    }

    fn eval(&self, n: usize) -> Option<String> {
        self.g.nodes[n].eval.map(|e| e.text())
    }

    /// "12.Sf3" / "12...Sf6" / "Sf6".
    fn numbered(&self, n: usize, force: bool) -> String {
        let num = self.g.move_number(n);
        if self.g.is_white(n) {
            format!("{num}.{}", self.san(n))
        } else if force {
            format!("{num}...{}", self.san(n))
        } else {
            self.san(n)
        }
    }

    fn trunk_text(&self) -> String {
        let mut s = Vec::new();
        for (i, &n) in self.rep.trunk.iter().enumerate() {
            s.push(self.numbered(n, i == 0));
        }
        s.join(" ")
    }

    fn new_page(&self, pages: &mut Vec<Page>) {
        let mut p = Page::default();
        let eco = self.g.header("ECO").unwrap_or("");
        p.text(MARGIN, TOP + 10.0, Font::Regular, 11.0, 0.0, eco);
        let trunk = self.trunk_text();
        let w = width(&trunk, Font::Regular, 11.0);
        p.text((PAGE_W - w) / 2.0, TOP + 10.0, Font::Regular, 11.0, 0.0, &trunk);
        p.line(MARGIN, TOP + 17.0, PAGE_W - MARGIN, TOP + 17.0, 0.6, 0.55);
        // footer
        let num = format!("{}", pages.len() + 1);
        p.text(MARGIN, PAGE_H - 16.0, Font::Regular, 7.5, 0.45, &self.opt.title);
        let w = width(&num, Font::Regular, 8.0);
        p.text(PAGE_W - MARGIN - w, PAGE_H - 16.0, Font::Regular, 8.0, 0.45, &num);
        pages.push(p);
    }

    // ---------------------------------------------------------------- table

    fn table(&self, pages: &mut Vec<Page>) {
        let ncols = (self.rep.last_col - self.rep.first_col + 1) as f32;
        let x0 = MARGIN + ROWNUM_W;
        let col_w = (PAGE_W - MARGIN - x0) / ncols;
        let head_y = TOP + 36.0;
        let first_row_y = head_y + 12.0;
        let per_page = (((BOTTOM - first_row_y) / ROW_H).floor() as usize).max(1);

        for (r, row) in self.rep.rows.iter().enumerate() {
            if r % per_page == 0 {
                self.new_page(pages);
                let p = pages.last_mut().unwrap();
                for c in 0..ncols as u32 {
                    let label = format!("{}", self.rep.first_col + c);
                    p.text(x0 + c as f32 * col_w, head_y, Font::Regular, MOVE_SIZE, 0.0, &label);
                }
            }
            let p = pages.last_mut().unwrap();
            let y = first_row_y + (r % per_page) as f32 * ROW_H;
            p.text(MARGIN, y + 14.0, Font::Bold, MOVE_SIZE, 0.0, &format!("{}", r + 1));
            for cell in &row.cells {
                let col = (self.g.move_number(cell.node) - self.rep.first_col) as f32;
                let x = x0 + col * col_w;
                let base = if self.g.is_white(cell.node) { y + 14.0 } else { y + 27.0 };
                if cell.dotted {
                    p.text(x, base, Font::Regular, MOVE_SIZE, 0.0, ". . .");
                    continue;
                }
                let san = self.san(cell.node);
                p.text(x, base, Font::Regular, MOVE_SIZE, 0.0, &san);
                let mut cx = x + width(&san, Font::Regular, MOVE_SIZE);
                if let Some(k) = cell.note {
                    let s = format!("{}", k + 1);
                    p.text(cx + 0.5, base - 4.0, Font::Regular, SUP_SIZE, 0.0, &s);
                    cx += 0.5 + width(&s, Font::Regular, SUP_SIZE);
                }
                if let Some(e) = self.eval(cell.node) {
                    p.text(cx + 2.5, base, Font::Regular, EVAL_SIZE, GRAY_EVAL, &e);
                }
            }
        }
    }

    // ---------------------------------------------------------------- notes

    fn notes(&self, pages: &mut Vec<Page>) {
        let mut w = NoteWriter { ctx: self, paras: Vec::new(), pending: Some(0), need_num: true };
        for (k, note) in self.rep.notes.iter().enumerate() {
            for (i, &line) in note.lines.iter().enumerate() {
                w.new_para(0);
                if i == 0 {
                    w.word(&format!("{}", k + 1), Font::Bold, false, false);
                }
                w.top_line(line);
            }
        }
        if !w.paras.is_empty() {
            layout_columns(self, pages, &w.paras);
        }
    }
}

struct Word {
    text: String,
    font: Font,
    underline: bool,
    gray: f32,
    /// Attached to the previous word (no space, no line break before it).
    glue: bool,
}

struct Para {
    indent: usize,
    words: Vec<Word>,
}

struct NoteWriter<'a> {
    ctx: &'a Ctx<'a>,
    paras: Vec<Para>,
    pending: Option<usize>,
    need_num: bool,
}

impl NoteWriter<'_> {
    fn new_para(&mut self, indent: usize) {
        self.pending = Some(indent);
        self.need_num = true;
    }

    fn push(&mut self, w: Word) {
        if let Some(indent) = self.pending.take() {
            self.paras.push(Para { indent, words: Vec::new() });
        }
        self.paras.last_mut().unwrap().words.push(w);
    }

    fn word(&mut self, text: &str, font: Font, underline: bool, glue: bool) {
        self.push(Word { text: text.to_string(), font, underline, gray: 0.0, glue });
    }

    /// A move with its evaluation.
    fn mv(&mut self, n: usize, font: Font, underline: bool) {
        let t = self.ctx.numbered(n, self.need_num);
        self.word(&t, font, underline, false);
        if let Some(e) = self.ctx.eval(n) {
            self.push(Word { text: e, font: Font::Regular, underline: false, gray: 0.0, glue: false });
        }
        self.need_num = false;
    }

    /// Footnote line: the main moves; every alternative goes into its own "[ ... ]" block.
    fn top_line(&mut self, start: usize) {
        let g = self.ctx.g;
        self.mv(start, Font::Regular, false);
        let mut cur = start;
        while let Some((&c0, rest)) = g.children(cur).split_first() {
            self.mv(c0, Font::Regular, false);
            for &o in rest {
                self.new_para(0);
                self.word("[", Font::Regular, false, false);
                self.var_line(o, String::new(), 0, false);
                self.close("]");
            }
            if !rest.is_empty() {
                self.new_para(0);
            }
            cur = c0;
        }
    }

    /// Line inside a block: leaf alternatives inline "( ...; ... )", real
    /// branchings as labelled sub-lines A) B) / A1) A2) / A1a) ...
    fn var_line(&mut self, start: usize, prefix: String, depth: usize, underline_first: bool) {
        let g = self.ctx.g;
        self.mv(start, Font::Regular, underline_first);
        let mut cur = start;
        loop {
            let ch = g.children(cur);
            match ch.len() {
                0 => return,
                1 => {
                    self.mv(ch[0], Font::Regular, false);
                    cur = ch[0];
                }
                _ => {
                    if ch[1..].iter().all(|&c| g.children(c).is_empty()) {
                        self.mv(ch[0], Font::Regular, false);
                        self.word("(", Font::Italic, false, false);
                        for (k, &o) in ch[1..].iter().enumerate() {
                            if k > 0 {
                                self.word(";", Font::Italic, false, true);
                            }
                            self.need_num = true;
                            self.mv(o, Font::Italic, false);
                        }
                        self.word(")", Font::Italic, false, false);
                        self.need_num = true;
                        cur = ch[0];
                    } else {
                        for (k, &c) in ch.iter().enumerate() {
                            let label = format!("{prefix}{}", label_part(depth, k));
                            self.new_para(depth);
                            self.word(&format!("{label})"), Font::Bold, true, false);
                            self.var_line(c, label, depth + 1, true);
                            self.semicolon();
                        }
                        return;
                    }
                }
            }
        }
    }

    /// End of a labelled sub-line (nested sub-lines already end with one).
    fn semicolon(&mut self) {
        if !self.paras.last().is_some_and(|p| p.words.last().is_some_and(|w| w.text == ";")) {
            self.word(";", Font::Regular, false, true);
        }
    }

    fn close(&mut self, s: &str) {
        if let Some(p) = self.paras.last_mut() {
            if p.words.last().is_some_and(|w| w.text == ";") {
                p.words.pop();
            }
        }
        self.pending = None;
        self.word(s, Font::Regular, false, false);
    }
}

/// Label part per nesting depth: A B C / 1 2 3 / a b c / 1 2 3 ...
fn label_part(depth: usize, k: usize) -> String {
    let letter = |base: u8| {
        let s = ((base + (k % 26) as u8) as char).to_string();
        if k >= 26 { format!("{s}{}", k / 26) } else { s }
    };
    match depth {
        0 => letter(b'A'),
        2 => letter(b'a'),
        _ => format!("{}", k + 1),
    }
}

/// Flow the paragraphs into two columns per page.
fn layout_columns(ctx: &Ctx, pages: &mut Vec<Page>, paras: &[Para]) {
    let col_w = (PAGE_W - 2.0 * MARGIN - GUTTER) / 2.0;
    let col_x = [MARGIN, MARGIN + col_w + GUTTER];
    let top = TOP + 36.0;
    let space = width(" ", Font::Regular, NOTE_SIZE);

    let mut col = 0;
    let mut y = top;
    ctx.new_page(pages);
    let mid = MARGIN + col_w + GUTTER / 2.0;
    pages.last_mut().unwrap().line(mid, top - 8.0, mid, BOTTOM, 0.5, 0.6);

    let emit = |line: &[(f32, &Word)], indent: f32, pages: &mut Vec<Page>, col: &mut usize, y: &mut f32| {
        if *y + NOTE_LINE > BOTTOM + 4.0 {
            if *col == 0 {
                *col = 1;
            } else {
                *col = 0;
                ctx.new_page(pages);
                pages.last_mut().unwrap().line(mid, top - 8.0, mid, BOTTOM, 0.5, 0.6);
            }
            *y = top;
        }
        let p = pages.last_mut().unwrap();
        let x0 = col_x[*col] + indent;
        for &(dx, w) in line {
            p.text(x0 + dx, *y, w.font, NOTE_SIZE, w.gray, &w.text);
            if w.underline {
                let tw = width(&w.text, w.font, NOTE_SIZE);
                p.line(x0 + dx, *y + 1.4, x0 + dx + tw, *y + 1.4, 0.5, 0.0);
            }
        }
        *y += NOTE_LINE;
    };

    for para in paras {
        let indent = para.indent as f32 * INDENT;
        let avail = col_w - indent;
        // units = words glued together
        let mut units: Vec<Vec<&Word>> = Vec::new();
        for w in &para.words {
            match units.last_mut() {
                Some(u) if w.glue => u.push(w),
                _ => units.push(vec![w]),
            }
        }
        let mut line: Vec<(f32, &Word)> = Vec::new();
        let mut x = 0.0;
        for u in units {
            let uw: f32 = u.iter().map(|w| width(&w.text, w.font, NOTE_SIZE)).sum();
            let start = if line.is_empty() { 0.0 } else { x + space };
            if !line.is_empty() && start + uw > avail {
                emit(&line, indent, pages, &mut col, &mut y);
                line.clear();
                x = 0.0;
            } else {
                x = start;
            }
            for w in u {
                line.push((x, w));
                x += width(&w.text, w.font, NOTE_SIZE);
            }
        }
        if !line.is_empty() {
            emit(&line, indent, pages, &mut col, &mut y);
        }
    }
}

pub fn render(g: &Game, rep: &Repertoire, opt: &Options) -> Vec<Page> {
    let ctx = Ctx { g, opt, rep };
    let mut pages = Vec::new();
    ctx.table(&mut pages);
    ctx.notes(&mut pages);
    pages
}
