# pgnrepertoireprint

Rust CLI (edition 2024, **no external crates**) that turns an analysed PGN tree into a
ChessBase-style repertoire PDF.

- `src/pgn.rs`: PGN reader (first game, nested variations). Keeps only SAN and the eval from each move comment.
- `src/repertoire.rs`: trunk, greedy row selection (largest subtree first), dots, footnote assignment.
- `src/render.rs`: page layout (table pages, then two-column notes), notation (de/en), note text rules.
- `src/pdf.rs` + `src/metrics.rs`: minimal PDF writer with the base-14 Helvetica fonts (WinAnsi widths).

Rules requested by the user:
- Table columns are fixed up to move 12 (`--last-move`).
- No annotation: only moves and their evaluation.
- Every move gets its evaluation rounded to **one decimal**.
- German piece letters are the default.

Build: `./build.sh` → `dist/pgnrepertoireprint.exe` + `dist/pgnrepertoireprint-linux`.
Test: `cargo test`. To check the output visually, render pages to PNG (e.g. with pymupdf).
