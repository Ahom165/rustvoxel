// Classic 5x7 pixel font (proportional widths) with composed French accents.
// Pure data + layout: drawing happens in ui.rs on top of the HUD quad pipeline.
//
// Rows are top-to-bottom, bit 4 = leftmost pixel. Accented glyphs are
// composed at lookup time (base letter pushed down one row + accent marks),
// so the table only holds ASCII.

pub const ROWS: usize = 8;

const fn g(rows: [u8; 7]) -> [u8; 7] {
    rows
}

const SPACE: [u8; 7] = g([0, 0, 0, 0, 0, 0, 0]);

pub const GLYPHS: [(char, [u8; 7]); 95] = [
    (' ', SPACE),
    ('!', g([0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100])),
    ('"', g([0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000])),
    ('#', g([0b01010, 0b10101, 0b11111, 0b01010, 0b11111, 0b10101, 0b01010])),
    ('$', g([0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100])),
    ('%', g([0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011])),
    ('&', g([0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101])),
    ('\'', g([0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000])),
    ('(', g([0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010])),
    (')', g([0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000])),
    ('*', g([0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000])),
    ('+', g([0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000])),
    (',', g([0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b00100, 0b01000])),
    ('-', g([0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000])),
    ('.', g([0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100])),
    ('/', g([0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000])),
    ('0', g([0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110])),
    ('1', g([0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110])),
    ('2', g([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111])),
    ('3', g([0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110])),
    ('4', g([0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010])),
    ('5', g([0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110])),
    ('6', g([0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110])),
    ('7', g([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000])),
    ('8', g([0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110])),
    ('9', g([0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100])),
    (':', g([0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000])),
    (';', g([0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b00100, 0b01000])),
    ('<', g([0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010])),
    ('=', g([0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000])),
    ('>', g([0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000])),
    ('?', g([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100])),
    ('@', g([0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110])),
    ('A', g([0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001])),
    ('B', g([0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110])),
    ('C', g([0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110])),
    ('D', g([0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100])),
    ('E', g([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111])),
    ('F', g([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000])),
    ('G', g([0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111])),
    ('H', g([0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001])),
    ('I', g([0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110])),
    ('J', g([0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100])),
    ('K', g([0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001])),
    ('L', g([0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111])),
    ('M', g([0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001])),
    ('N', g([0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001])),
    ('O', g([0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110])),
    ('P', g([0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000])),
    ('Q', g([0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101])),
    ('R', g([0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001])),
    ('S', g([0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110])),
    ('T', g([0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100])),
    ('U', g([0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110])),
    ('V', g([0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100])),
    ('W', g([0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010])),
    ('X', g([0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001])),
    ('Y', g([0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100])),
    ('Z', g([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111])),
    ('[', g([0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110])),
    ('\\', g([0b10000, 0b01000, 0b01000, 0b00100, 0b00010, 0b00010, 0b00001])),
    (']', g([0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110])),
    ('^', g([0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000])),
    ('_', g([0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111])),
    ('`', g([0b01000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000])),
    ('a', g([0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111])),
    ('b', g([0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110])),
    ('c', g([0b00000, 0b00000, 0b01111, 0b10000, 0b10000, 0b10000, 0b01111])),
    ('d', g([0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111])),
    ('e', g([0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110])),
    ('f', g([0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000])),
    ('g', g([0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110])),
    ('h', g([0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001])),
    ('i', g([0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110])),
    ('j', g([0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100])),
    ('k', g([0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010])),
    ('l', g([0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110])),
    ('m', g([0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101])),
    ('n', g([0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001])),
    ('o', g([0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110])),
    ('p', g([0b00000, 0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000])),
    ('q', g([0b00000, 0b00000, 0b01111, 0b10001, 0b01111, 0b00001, 0b00001])),
    ('r', g([0b00000, 0b00000, 0b01110, 0b10001, 0b10000, 0b10000, 0b10000])),
    ('s', g([0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110])),
    ('t', g([0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000, 0b00110])),
    ('u', g([0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101])),
    ('v', g([0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100])),
    ('w', g([0b00000, 0b00000, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010])),
    ('x', g([0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001])),
    ('y', g([0b00000, 0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110])),
    ('z', g([0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111])),
    ('{', g([0b00110, 0b00100, 0b00100, 0b01100, 0b00100, 0b00100, 0b00110])),
    ('|', g([0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100])),
    ('}', g([0b01100, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01100])),
    ('~', g([0b00000, 0b00000, 0b01000, 0b10101, 0b00010, 0b00000, 0b00000])),
];

#[derive(Clone, Copy, PartialEq)]
pub enum Accent {
    Acute,
    Grave,
    Circumflex,
    Umlaut,
    Cedilla,
    Tilde,
}

/// Composed accented characters -> (base char, accent).
const ACCENTS: [(char, char, Accent); 23] = [
    ('à', 'a', Accent::Grave),
    ('â', 'a', Accent::Circumflex),
    ('ä', 'a', Accent::Umlaut),
    ('é', 'e', Accent::Acute),
    ('è', 'e', Accent::Grave),
    ('ê', 'e', Accent::Circumflex),
    ('ë', 'e', Accent::Umlaut),
    ('î', 'i', Accent::Circumflex),
    ('ï', 'i', Accent::Umlaut),
    ('ô', 'o', Accent::Circumflex),
    ('ö', 'o', Accent::Umlaut),
    ('ù', 'u', Accent::Grave),
    ('û', 'u', Accent::Circumflex),
    ('ü', 'u', Accent::Umlaut),
    ('ç', 'c', Accent::Cedilla),
    ('œ', 'o', Accent::Tilde),
    ('É', 'E', Accent::Acute),
    ('È', 'E', Accent::Grave),
    ('Ê', 'E', Accent::Circumflex),
    ('À', 'A', Accent::Grave),
    ('Â', 'A', Accent::Circumflex),
    ('Ç', 'C', Accent::Cedilla),
    ('Ô', 'O', Accent::Circumflex),
];

/// Lookup a glyph: returns up to 8 rows (row 0 first) and its pixel width.
pub fn glyph(c: char) -> Option<([u8; ROWS], usize)> {
    // accent composition
    for (ch, base, acc) in ACCENTS.iter() {
        if *ch == c {
            return compose(*base, *acc);
        }
    }
    for (ch, rows) in GLYPHS.iter() {
        if *ch == c {
            let mut r = [0u8; ROWS];
            r[..7].copy_from_slice(rows);
            return Some((r, used_width(rows)));
        }
    }
    None
}

fn used_width(rows: &[u8; 7]) -> usize {
    let mut w = 0usize;
    for (i, &row) in rows.iter().enumerate() {
        for b in 0..5 {
            if row & (1 << (4 - b)) != 0 && b + 1 > w {
                w = b + 1;
            }
        }
        let _ = i;
    }
    w.max(1)
}

fn compose(base: char, acc: Accent) -> Option<([u8; ROWS], usize)> {
    let (_, rows) = GLYPHS.iter().find(|(c, _)| *c == base)?;
    let w = used_width(rows);
    let mut r = [0u8; ROWS];
    match acc {
        Accent::Cedilla => {
            r[..7].copy_from_slice(rows);
            r[7] = 0b00010;
        }
        _ => {
            // push the base glyph down one row, accent on the top row
            r[0] = match acc {
                Accent::Acute => 0b00010,
                Accent::Grave => 0b01000,
                Accent::Circumflex => 0b01010,
                Accent::Umlaut => 0b01010,
                Accent::Tilde => 0b01101,
                _ => 0,
            };
            for k in 0..7 {
                r[k + 1] = rows[k];
            }
        }
    }
    Some((r, w))
}

/// Advance width of one character in pixels (glyph + 1 px spacing).
pub fn char_width(c: char) -> usize {
    glyph(c).map(|(_, w)| w + 1).unwrap_or(5)
}

/// Width of a string in pixels (scale 1).
pub fn width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

// ------------------------------------------------------------------- tests
#[cfg(test)]
mod tests {
    use super::*;

    fn art(c: char) -> String {
        match glyph(c) {
            Some((rows, w)) => {
                let mut s = String::new();
                for row in rows.iter() {
                    for b in 0..5 {
                        s.push(if b < w && row & (1 << (4 - b)) != 0 { '#' } else { '.' });
                    }
                    s.push('\n');
                }
                s
            }
            None => "?".into(),
        }
    }

    #[test]
    fn all_ascii_present() {
        for c in (32..127).map(|i| i as u8 as char) {
            assert!(glyph(c).is_some(), "missing glyph {c:?}");
        }
    }

    #[test]
    fn french_accents_compose() {
        for (ch, base, _) in ACCENTS.iter() {
            let (rows, _) = glyph(*ch).unwrap_or(([0; 8], 0));
            let (brows, _) = glyph(*base).unwrap_or(([0; 8], 0));
            let on = |r: &[u8; 8]| r.iter().filter(|&&x| x != 0).count();
            // composed glyph must have its marks (same or more ink than base,
            // modulo the tiny cedilla) - a light sanity check:
            assert!(on(&rows) >= on(&brows) - 1, "accent {ch} lost ink");
        }
    }

    #[test]
    #[ignore] // cargo test font_eyeball -- --ignored --nocapture
    fn font_eyeball() {
        let text = "RustVoxel 0.6 !éèêàçÇÉ?";
        let mut out = String::new();
        let rows: Vec<Vec<char>> = (0..ROWS)
            .map(|r| {
                text.chars()
                    .flat_map(|c| {
                        let (g, w) = glyph(c).unwrap_or(([0; 8], 1));
                        (0..w)
                            .map(move |b| {
                                if g[r] & (1 << (4 - b)) != 0 {
                                    '#'
                                } else {
                                    '.'
                                }
                            })
                            .chain(std::iter::once(' '))
                    })
                    .collect()
            })
            .collect();
        for r in rows {
            for c in r {
                out.push(c);
            }
            out.push('\n');
        }
        println!("{out}");
    }
}
