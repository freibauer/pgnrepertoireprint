//! pgnrepertoireprint: prints an analysed PGN opening tree as a repertoire PDF
//! in the style of ChessBase (overview table + footnotes), with a fixed last
//! move and the evaluation of every move rounded to one decimal.

mod metrics;
mod pdf;
mod pgn;
mod render;
mod repertoire;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
pgnrepertoireprint - repertoire PDF from an analysed PGN tree

Usage: pgnrepertoireprint <input.pgn> [options]

Options:
  -o, --output <file.pdf>  output file (default: input name with .pdf)
  --last-move <n>          last move shown in the table (default 12)
  --rows <n>               number of main lines in the overview table (default 30)
  --lang <de|en>           piece letters: de = K D T L S (default), en = K Q R B N
  --title <text>           footer text (default: PGN Event tag or file name)
  --row <moves>            put this line into the table (repeatable), e.g.
                           --row \"1.d4 d5 2.e4 dxe4 3.Nc3 e5 4.d5 f5\" (English or German letters)
  -h, --help               this help

Evaluations are read from the move comments ({ +0.35 }, { #3 } or [%eval 0.35])
and printed with one decimal. All other comments and symbols are ignored.";

struct Args {
    input: PathBuf,
    output: Option<PathBuf>,
    last_move: u32,
    rows: usize,
    german: bool,
    title: Option<String>,
    rows_forced: Vec<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let mut a = Args { input: PathBuf::new(), output: None, last_move: 12, rows: 30, german: true, title: None, rows_forced: Vec::new() };
    let mut input = None;
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "-o" | "--output" => a.output = Some(value(&arg)?.into()),
            "--last-move" => a.last_move = value(&arg)?.parse().map_err(|_| "--last-move: number expected")?,
            "--rows" => a.rows = value(&arg)?.parse().map_err(|_| "--rows: number expected")?,
            "--lang" => {
                a.german = match value(&arg)?.as_str() {
                    "de" => true,
                    "en" => false,
                    _ => return Err("--lang: de or en".into()),
                }
            }
            "--title" => a.title = Some(value(&arg)?),
            "--row" => a.rows_forced.push(value(&arg)?),
            s if s.starts_with('-') => return Err(format!("unknown option {s}")),
            _ if input.is_none() => input = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument {arg}")),
        }
    }
    a.input = input.ok_or_else(String::new)?;
    Ok(a)
}

fn run(a: Args) -> Result<(), String> {
    let bytes = std::fs::read(&a.input).map_err(|e| format!("{}: {e}", a.input.display()))?;
    // PGN files are UTF-8 or Latin-1
    let text = String::from_utf8(bytes).unwrap_or_else(|e| e.into_bytes().iter().map(|&b| b as char).collect());
    let game = pgn::parse(&text).map_err(|e| format!("{}: {e}", a.input.display()))?;

    let mut forced = Vec::new();
    for line in &a.rows_forced {
        forced.push(find_line(&game, line)?);
    }
    let rep = repertoire::build(&game, a.last_move, a.rows, &forced);
    let title = a.title.clone().unwrap_or_else(|| {
        game.header("Event").map(str::to_string).unwrap_or_else(|| {
            a.input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
        })
    });
    let opt = render::Options { german: a.german, title: title.clone() };
    let pages = render::render(&game, &rep, &opt);
    let out = a.output.clone().unwrap_or_else(|| a.input.with_extension("pdf"));
    std::fs::write(&out, pdf::write(&pages, &title)).map_err(|e| format!("{}: {e}", out.display()))?;
    println!(
        "{}: {} moves, {} table rows, {} footnotes, {} pages",
        out.display(),
        game.nodes.len() - 1,
        rep.rows.len(),
        rep.notes.len(),
        pages.len()
    );
    Ok(())
}

/// Node at the end of a move list like "1.d4 d5 2.e4" (English or German piece letters).
fn find_line(g: &pgn::Game, line: &str) -> Result<usize, String> {
    let german = |s: &str| -> String {
        s.chars().map(|c| match c { 'S' => 'N', 'L' => 'B', 'T' => 'R', 'D' => 'Q', c => c }).collect()
    };
    let clean = |s: &str| s.trim_end_matches(['+', '#', '!', '?']).replace('0', "O").to_string();
    let mut node = 0;
    for tok in line.split_whitespace() {
        let mv = match tok.rfind('.') {
            Some(p) => &tok[p + 1..],
            None => tok,
        };
        if mv.is_empty() {
            continue;
        }
        let want = clean(mv);
        let found = g.children(node).iter().copied().find(|&c| {
            let have = clean(&g.nodes[c].san);
            have == want || have == german(&want)
        });
        node = found.ok_or_else(|| format!("--row: move {mv} not found in \"{line}\""))?;
    }
    if node == 0 {
        return Err(format!("--row: no moves in \"{line}\""));
    }
    Ok(node)
}

fn main() -> ExitCode {
    match parse_args() {
        Err(e) => {
            if !e.is_empty() {
                eprintln!("error: {e}\n");
            }
            eprintln!("{USAGE}");
            if e.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(2) }
        }
        Ok(a) => match run(a) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
    }
}
