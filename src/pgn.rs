//! PGN reader for one game with nested variations. Only what the repertoire
//! print needs is kept: the move tree (SAN text), and per move the engine
//! evaluation found in its comment. All other annotation (text, NAGs) is dropped.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eval {
    /// Centipawns from White's point of view.
    Cp(i32),
    /// Mate in n moves (negative: Black mates).
    Mate(i32),
}

impl Eval {
    /// One decimal, rounded half away from zero: +0.35 -> "+0.4", -1.30 -> "-1.3".
    pub fn text(self) -> String {
        match self {
            Eval::Mate(n) => format!("#{n}"),
            Eval::Cp(cp) => {
                let tenths = (cp.abs() + 5) / 10;
                let sign = if tenths == 0 { "" } else if cp < 0 { "-" } else { "+" };
                format!("{sign}{}.{}", tenths / 10, tenths % 10)
            }
        }
    }
}

#[derive(Debug)]
pub struct Node {
    /// SAN as in the file (castling normalised to O-O); empty for the root.
    pub san: String,
    pub parent: usize,
    pub children: Vec<usize>,
    /// Half-moves from the start position (root 0, first move 1).
    pub ply: usize,
    pub eval: Option<Eval>,
}

pub struct Game {
    pub headers: Vec<(String, String)>,
    /// nodes[0] is the root (start position); children always have higher indices.
    pub nodes: Vec<Node>,
    black_first: bool,
    start_move: u32,
}

impl Game {
    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty() && *v != "?")
    }

    pub fn children(&self, n: usize) -> &[usize] {
        &self.nodes[n].children
    }

    pub fn parent(&self, n: usize) -> usize {
        self.nodes[n].parent
    }

    fn half(&self, n: usize) -> usize {
        self.nodes[n].ply - 1 + self.black_first as usize
    }

    /// Full-move number of the move leading to node `n`.
    pub fn move_number(&self, n: usize) -> u32 {
        self.start_move + (self.half(n) / 2) as u32
    }

    /// Whether the move leading to node `n` is a White move.
    pub fn is_white(&self, n: usize) -> bool {
        self.half(n) % 2 == 0
    }
}

/// Engine evaluation from a comment: `[%eval 0.35]`, or a comment that starts
/// with a value such as `+0.35`, `-1.30`, `0.00`, `#3`, `#-3`.
fn parse_eval(comment: &str) -> Option<Eval> {
    let value = if let Some(p) = comment.find("[%eval") {
        comment[p + 6..].trim_start().split([']', ' ', ',']).next()?
    } else {
        comment.split_whitespace().next()?
    };
    if let Some(m) = value.strip_prefix('#') {
        return m.trim_start_matches('+').parse().ok().map(Eval::Mate);
    }
    let v = value.trim_start_matches('+');
    if v.is_empty() || !v.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
        return None;
    }
    let f: f64 = v.parse().ok()?;
    Some(Eval::Cp((f * 100.0).round() as i32))
}

/// Parse the first game of a PGN text.
pub fn parse(text: &str) -> Result<Game, String> {
    let chars: Vec<char> = text.trim_start_matches('\u{feff}').chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut headers = Vec::new();
    let mut nodes = vec![Node { san: String::new(), parent: 0, children: Vec::new(), ply: 0, eval: None }];
    let mut cur = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut after_move = false;
    let mut seen_move = false;
    let at_line_start = |i: usize| i == 0 || chars[i - 1] == '\n';

    while i < n {
        let c = chars[i];
        match c {
            '[' if stack.is_empty() => {
                if seen_move {
                    break; // next game
                }
                let start = i + 1;
                while i < n && chars[i] != ']' {
                    if chars[i] == '"' {
                        i += 1;
                        while i < n && chars[i] != '"' {
                            if chars[i] == '\\' {
                                i += 1;
                            }
                            i += 1;
                        }
                    }
                    i += 1;
                }
                let tag: String = chars[start..i.min(n)].iter().collect();
                if let Some((k, v)) = tag.trim().split_once(char::is_whitespace) {
                    let v = v.trim().trim_matches('"').replace("\\\"", "\"").replace("\\\\", "\\");
                    headers.push((k.to_string(), v));
                }
                i += 1;
            }
            '{' => {
                let start = i + 1;
                while i < n && chars[i] != '}' {
                    i += 1;
                }
                let comment: String = chars[start..i.min(n)].iter().collect();
                if after_move && nodes[cur].eval.is_none() {
                    nodes[cur].eval = parse_eval(&comment);
                }
                i += 1;
            }
            ';' => {
                while i < n && chars[i] != '\n' {
                    i += 1;
                }
            }
            '%' if at_line_start(i) => {
                while i < n && chars[i] != '\n' {
                    i += 1;
                }
            }
            '(' => {
                if cur == 0 {
                    return Err("variation before the first move".into());
                }
                stack.push(cur);
                cur = nodes[cur].parent;
                after_move = false;
                i += 1;
            }
            ')' => {
                if let Some(c) = stack.pop() {
                    cur = c;
                }
                after_move = false;
                i += 1;
            }
            '$' => {
                i += 1;
                while i < n && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            c if c.is_whitespace() => i += 1,
            _ => {
                let start = i;
                while i < n && !chars[i].is_whitespace() && !"{}();[".contains(chars[i]) {
                    i += 1;
                }
                let token: String = chars[start..i].iter().collect();
                if matches!(token.as_str(), "1-0" | "0-1" | "1/2-1/2" | "*") {
                    after_move = false;
                    continue;
                }
                // strip a move number prefix "12." / "12..." (castling "0-0" has no dot)
                let mut san = token.as_str();
                if san.starts_with(|c: char| c.is_ascii_digit()) && !san.starts_with("0-0") {
                    match san.find('.') {
                        Some(p) => san = san[p..].trim_start_matches('.'),
                        None => continue, // stray number
                    }
                }
                let san = san.trim_end_matches(['!', '?']);
                if san.is_empty() {
                    continue;
                }
                let san = if san.starts_with("0-0") { san.replace('0', "O") } else { san.to_string() };
                let ply = nodes[cur].ply + 1;
                let existing = nodes[cur].children.iter().copied().find(|&c| nodes[c].san == san);
                cur = match existing {
                    Some(c) => c,
                    None => {
                        let id = nodes.len();
                        nodes.push(Node { san, parent: cur, children: Vec::new(), ply, eval: None });
                        nodes[cur].children.push(id);
                        id
                    }
                };
                after_move = true;
                seen_move = true;
            }
        }
    }
    if nodes.len() == 1 {
        return Err("no moves found".into());
    }

    let (mut black_first, mut start_move) = (false, 1u32);
    if let Some((_, fen)) = headers.iter().find(|(k, _)| k == "FEN") {
        let f: Vec<&str> = fen.split_whitespace().collect();
        black_first = f.get(1) == Some(&"b");
        start_move = f.get(5).and_then(|s| s.parse().ok()).unwrap_or(1).max(1);
    }
    Ok(Game { headers, nodes, black_first, start_move })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_and_evals() {
        let g = parse("[Event \"x\"]\n\n1. d4 { +0.30 } d5 { +0.35 } 2. e4 (2. c4 { +0.40 } e6 { 0.38 }) 2... dxe4 { -0.10 } *").unwrap();
        assert_eq!(g.header("Event"), Some("x"));
        let d4 = g.children(0)[0];
        let d5 = g.children(d4)[0];
        assert_eq!(g.children(d5).len(), 2);
        let c4 = g.children(d5)[1];
        assert_eq!(g.nodes[c4].san, "c4");
        assert_eq!(g.nodes[c4].eval, Some(Eval::Cp(40)));
        assert_eq!(g.move_number(c4), 2);
        assert!(g.is_white(c4));
        assert!(!g.is_white(d5));
    }

    #[test]
    fn eval_formats() {
        assert_eq!(Eval::Cp(35).text(), "+0.4");
        assert_eq!(Eval::Cp(-130).text(), "-1.3");
        assert_eq!(Eval::Cp(-125).text(), "-1.3");
        assert_eq!(Eval::Cp(-4).text(), "0.0");
        assert_eq!(Eval::Mate(-3).text(), "#-3");
        assert_eq!(parse_eval("[%eval -0.62]"), Some(Eval::Cp(-62)));
        assert_eq!(parse_eval(" #3 "), Some(Eval::Mate(3)));
        assert_eq!(parse_eval("good move"), None);
    }

    #[test]
    fn castling_and_numbers() {
        let g = parse("1.e4 e5 2.Nf3 Nc6 3.Bc4 Bc5 4.0-0 Nf6 *").unwrap();
        let last = g.nodes.len() - 2;
        assert_eq!(parse("1.e4 e5 2.O-O-O *").unwrap().nodes[3].san, "O-O-O");
        assert_eq!(g.nodes[last].san, "O-O");
        assert_eq!(g.move_number(last), 4);
    }
}
