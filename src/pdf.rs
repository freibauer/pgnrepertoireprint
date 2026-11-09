//! Minimal PDF writer: A4 landscape pages, the three standard fonts
//! Helvetica / Helvetica-Bold / Helvetica-Oblique (not embedded, WinAnsi
//! encoding), text and lines. Coordinates are in points with the origin at the
//! top-left corner (y grows downwards); they are flipped when written.

use crate::metrics::{HELVETICA, HELVETICA_BOLD};
use std::fmt::Write as _;

pub const PAGE_W: f32 = 841.89;
pub const PAGE_H: f32 = 595.28;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Font {
    Regular,
    Bold,
    Italic,
}

impl Font {
    fn resource(self) -> &'static str {
        match self {
            Font::Regular => "F1",
            Font::Bold => "F2",
            Font::Italic => "F3",
        }
    }
}

/// Text as WinAnsi bytes; characters outside Latin-1 become '?'.
pub fn encode(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if (32..=126).contains(&u) || (160..=255).contains(&u) { u as u8 } else { b'?' }
        })
        .collect()
}

/// Width of `s` in points.
pub fn width(s: &str, font: Font, size: f32) -> f32 {
    let table = if font == Font::Bold { &HELVETICA_BOLD } else { &HELVETICA };
    encode(s).iter().map(|&b| table[b as usize] as f32).sum::<f32>() * size / 1000.0
}

fn pdf_string(out: &mut Vec<u8>, s: &str) {
    out.push(b'(');
    for b in encode(s) {
        match b {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            32..=126 => out.push(b),
            _ => out.extend_from_slice(format!("\\{b:03o}").as_bytes()),
        }
    }
    out.push(b')');
}

#[derive(Default)]
pub struct Page {
    ops: Vec<u8>,
}

impl Page {
    /// Text with its baseline at `y` (from the top).
    pub fn text(&mut self, x: f32, y: f32, font: Font, size: f32, gray: f32, s: &str) {
        if s.is_empty() {
            return;
        }
        let mut head = String::new();
        let _ = write!(head, "BT /{} {:.2} Tf {:.3} g {:.2} {:.2} Td ", font.resource(), size, gray, x, PAGE_H - y);
        self.ops.extend_from_slice(head.as_bytes());
        pdf_string(&mut self.ops, s);
        self.ops.extend_from_slice(b" Tj ET\n");
    }

    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32, gray: f32) {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "{gray:.3} G {w:.2} w {x1:.2} {:.2} m {x2:.2} {:.2} l S",
            PAGE_H - y1,
            PAGE_H - y2
        );
        self.ops.extend_from_slice(s.as_bytes());
    }
}

/// Assemble the PDF file.
pub fn write(pages: &[Page], title: &str) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    let n_obj = 6 + 2 * pages.len();
    let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };

    // 1 catalog, 2 page tree, 3-5 fonts, 6 info, then (page, content) pairs
    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len()).map(|i| format!("{} 0 R", 7 + 2 * i)).collect();
    obj(
        &mut out,
        &mut offsets,
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), pages.len()).as_bytes(),
    );
    for base in ["Helvetica", "Helvetica-Bold", "Helvetica-Oblique"] {
        obj(
            &mut out,
            &mut offsets,
            format!("<< /Type /Font /Subtype /Type1 /BaseFont /{base} /Encoding /WinAnsiEncoding >>").as_bytes(),
        );
    }
    let mut info = b"<< /Producer (pgnrepertoireprint) /Title ".to_vec();
    pdf_string(&mut info, title);
    info.extend_from_slice(b" >>");
    obj(&mut out, &mut offsets, &info);

    for (i, page) in pages.iter().enumerate() {
        obj(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R >> >> /Contents {} 0 R >>",
                8 + 2 * i
            )
            .as_bytes(),
        );
        let mut body = format!("<< /Length {} >>\nstream\n", page.ops.len()).into_bytes();
        body.extend_from_slice(&page.ops);
        body.extend_from_slice(b"endstream");
        obj(&mut out, &mut offsets, &body);
    }

    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", n_obj + 1).as_bytes());
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R /Info 6 0 R >>\nstartxref\n{xref}\n%%EOF\n", n_obj + 1).as_bytes(),
    );
    out
}
