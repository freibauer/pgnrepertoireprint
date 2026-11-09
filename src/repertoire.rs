//! Turns the move tree into a ChessBase-style repertoire: the common trunk,
//! an overview table (rows = main lines, one column per full move up to a fixed
//! last move) and footnotes holding every line that is not a table row.

use crate::pgn::Game;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub struct Cell {
    pub node: usize,
    /// Same move as in the row above ("...").
    pub dotted: bool,
    /// Index into `Repertoire::notes`.
    pub note: Option<usize>,
}

pub struct Row {
    pub cells: Vec<Cell>,
}

pub struct Note {
    /// Lines printed in the footnote, each starting with this node's move:
    /// first the alternatives to the cell's move, then (if the row was cut at
    /// the last table move) the continuation after it.
    pub lines: Vec<usize>,
}

pub struct Repertoire {
    /// Moves common to all lines (shown in the header).
    pub trunk: Vec<usize>,
    pub first_col: u32,
    pub last_col: u32,
    pub rows: Vec<Row>,
    pub notes: Vec<Note>,
}

/// `forced`: nodes that must get their own table row (e.g. a line the user wants
/// to see in the overview); they are added right after the main line.
pub fn build(g: &Game, last_move: u32, max_rows: usize, forced: &[usize]) -> Repertoire {
    let n = g.nodes.len();

    // Trunk: follow the single line until the first branching.
    let mut trunk = Vec::new();
    let mut t = 0;
    while g.children(t).len() == 1 {
        t = g.children(t)[0];
        trunk.push(t);
    }
    if g.children(t).is_empty() {
        // one single line without any branching: show it as one table row
        trunk.clear();
        t = 0;
    }
    let first_col = g.move_number(g.children(t)[0]);
    let last_col = last_move.max(first_col);

    // Leaf counts (children have higher indices) and preorder positions.
    let mut leaves = vec![0usize; n];
    for i in (0..n).rev() {
        leaves[i] = if g.children(i).is_empty() { 1 } else { g.children(i).iter().map(|&c| leaves[c]).sum() };
    }
    let mut pre = vec![0usize; n];
    let mut stack = vec![0usize];
    let mut k = 0;
    while let Some(x) = stack.pop() {
        pre[x] = k;
        k += 1;
        stack.extend(g.children(x).iter().rev());
    }

    // Row through branch node b: trunk end -> b, then main continuation up to last_col.
    let row_path = |b: usize| -> Vec<usize> {
        let mut path = Vec::new();
        let mut x = b;
        while x != t {
            path.push(x);
            x = g.parent(x);
        }
        path.reverse();
        let mut x = b;
        while let Some(&c) = g.children(x).first() {
            if g.move_number(c) > last_col {
                break;
            }
            path.push(c);
            x = c;
        }
        path
    };

    // Greedy row selection: always open the alternative with the largest subtree.
    let mut in_row = vec![false; n];
    let mut paths: Vec<Vec<usize>> = Vec::new();
    let mut heap: BinaryHeap<(usize, Reverse<usize>, usize)> = BinaryHeap::new();
    let add_row = |b: usize, in_row: &mut Vec<bool>, heap: &mut BinaryHeap<_>, paths: &mut Vec<Vec<usize>>| {
        let path = row_path(b);
        for &x in &path {
            in_row[x] = true;
        }
        for &x in &path {
            for &c in g.children(g.parent(x)) {
                if !in_row[c] {
                    heap.push((leaves[c], Reverse(pre[c]), c));
                }
            }
        }
        paths.push(path);
    };
    add_row(g.children(t)[0], &mut in_row, &mut heap, &mut paths);
    for &f in forced {
        // the row starts at the first node of the line that is not yet in a row
        let mut chain = vec![f];
        while let Some(&x) = chain.last() {
            if x == t || x == 0 {
                break;
            }
            chain.push(g.parent(x));
        }
        if !chain.contains(&t) {
            continue; // inside the trunk
        }
        if let Some(&b) = chain.iter().rev().find(|&&x| x != t && x != 0 && !in_row[x]) {
            add_row(b, &mut in_row, &mut heap, &mut paths);
        }
        // follow the forced line itself to its end node, not only its main continuation
        if !in_row[f] {
            add_row(f, &mut in_row, &mut heap, &mut paths);
        }
    }
    while paths.len() < max_rows.max(1) {
        let Some((_, _, c)) = heap.pop() else { break };
        if !in_row[c] {
            add_row(c, &mut in_row, &mut heap, &mut paths);
        }
    }
    paths.sort_by_key(|p| pre[*p.last().unwrap()]);

    // Cells, dots and footnotes (numbered row by row, move by move).
    let mut noted = vec![false; n];
    let mut notes = Vec::new();
    let mut rows = Vec::new();
    for (r, path) in paths.iter().enumerate() {
        let mut cells = Vec::new();
        for (i, &x) in path.iter().enumerate() {
            let dotted = r > 0 && paths[r - 1].get(i) == Some(&x);
            let mut lines = Vec::new();
            if !dotted {
                let p = g.parent(x);
                if !noted[p] {
                    noted[p] = true;
                    lines.extend(g.children(p).iter().copied().filter(|&c| !in_row[c]));
                }
                if i + 1 == path.len() {
                    lines.extend_from_slice(g.children(x)); // cut at the last table move
                }
            }
            let note = if lines.is_empty() {
                None
            } else {
                notes.push(Note { lines });
                Some(notes.len() - 1)
            };
            cells.push(Cell { node: x, dotted, note });
        }
        rows.push(Row { cells });
    }

    Repertoire { trunk, first_col, last_col, rows, notes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pgn::parse;

    #[test]
    fn rows_and_notes() {
        let g = parse("1.d4 d5 2.e4 (2.c4 e6 (2...c6 3.Nf3) 3.Nc3) (2.Nf3) 2...dxe4 3.Nc3 *").unwrap();
        let r = build(&g, 12, 2, &[]);
        assert_eq!(r.trunk.len(), 2); // 1.d4 d5
        assert_eq!(r.first_col, 2);
        assert_eq!(r.rows.len(), 2);
        // second row shares nothing with the first (branch at move 2)
        assert!(!r.rows[1].cells[0].dotted);
        // 2.Nf3 and 2...c6 end up in footnotes
        let total: usize = r.notes.iter().map(|n| n.lines.len()).sum();
        assert_eq!(total, 2);
    }
}
