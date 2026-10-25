//! Headset controller glyphs in place of the PS2 pad icons (text bytes 0x10..0x19) of the three fonts.
//! Every text path (pause, freeze and vendor menus, help boxes, banners, subtitles, spatial panels) samples
//! the same FX textures and glyph tables, so the cells are repainted once, when the level HUD loads.
//! The glyphs follow the Touch / PICO layout of `game_input::button_path` and the VR controls that send each
//! native button (`game_input::sample`), so every icon names a control the player has.
use rc_formats::font::{Font, GlyphTable};
use rc_formats::texture::Texture;

/// A pad icon cell: 24×16 texels at the glyph's (u, v).
const CELL_W: usize = 24;
const CELL_H: usize = 16;
/// Raw GS alpha of an opaque texel.
const OPAQUE: f32 = 128.0;

#[derive(Clone, Copy)]
enum Shape {
    /// A round face button with its letter.
    Button(&'static str),
    /// A trigger or grip: its label in a bevelled key, like the PS2 shoulder icons.
    Key(&'static str),
    /// A thumbstick seen from above, its side's letter on the cap.
    Stick(&'static str),
    /// The same thumbstick with a press arrow above it.
    StickClick(&'static str),
}

/// The pad icon codes the headset answers, with the control that sends them.
const GLYPHS: [(u8, Shape); 10] = [
    // ✕: right A (jump / confirm).
    (0x10, Shape::Button("A")),
    // ○: right trigger (weapons, gadgets, the menu pointer's click).
    (0x11, Shape::Key("RT")),
    // △: right stick click (interact, quick select, back).
    (0x12, Shape::StickClick("R")),
    // □: right grip (the wrench).
    (0x13, Shape::Key("RG")),
    // L1: left grip in menus (previous map / planet / slot).
    (0x14, Shape::Key("LG")),
    // R1: left trigger (crouch, packs, flips; next map / planet / slot in menus).
    (0x15, Shape::Key("LT")),
    // L2: left X in play.
    (0x16, Shape::Button("X")),
    // R2: right B in play.
    (0x17, Shape::Button("B")),
    // Right analog stick.
    (0x18, Shape::Stick("R")),
    // Left analog stick.
    (0x19, Shape::Stick("L")),
];

/// Bold letters, 2-texel stems: (letter, width, rows; `#` = ink).
const LETTERS: [(char, usize, [&str; 9]); 7] = [
    ('A', 7, ["..###..", ".##.##.", "##...##", "##...##", "##...##", "#######", "##...##", "##...##", "##...##"]),
    ('X', 7, ["##...##", "##...##", ".##.##.", "..###..", "..###..", "..###..", ".##.##.", "##...##", "##...##"]),
    ('B', 7, ["######.", "##...##", "##...##", "##...##", "######.", "##...##", "##...##", "##...##", "######."]),
    ('R', 6, ["#####.", "##..##", "##..##", "##..##", "#####.", "##.##.", "##..##", "##..##", "##..##"]),
    ('T', 6, ["######", "######", "..##..", "..##..", "..##..", "..##..", "..##..", "..##..", "..##.."]),
    ('G', 6, [".####.", "##..##", "##....", "##....", "##.###", "##..##", "##..##", "##..##", ".####."]),
    ('L', 6, ["##....", "##....", "##....", "##....", "##....", "##....", "##....", "######", "######"]),
];
/// The stick caps' smaller letters: (letter, width, rows).
const SMALL_LETTERS: [(char, usize, [&str; 7]); 2] = [
    ('R', 5, ["####.", "##.##", "##.##", "####.", "##.#.", "##.##", "##.##"]),
    ('L', 5, ["##...", "##...", "##...", "##...", "##...", "#####", "#####"]),
];

/// Repaints the mapped pad icons of the fonts' FX textures and sets their advances.
pub(crate) fn apply(fx: &mut [Option<Texture>], glyphs: &mut [GlyphTable; 3]) {
    for font in [Font::Regular, Font::Small, Font::Large] {
        let Some(Some(texture)) = fx.get_mut(font.fx_texture()) else { continue };
        let table = &mut glyphs[font as usize];
        for (code, shape) in GLYPHS {
            let g = &mut table[code as usize];
            let (u, v) = (g.u as usize, g.v as usize);
            if g.advance == 0 || u + CELL_W > texture.width as usize || v + CELL_H > texture.height as usize {
                continue;
            }
            let mut cell = Cell { texture: &mut *texture, u, v };
            cell.clear();
            g.advance = paint(&mut cell, shape);
        }
    }
}

/// Draws the glyph in a cleared cell; returns its advance.
fn paint(cell: &mut Cell, shape: Shape) -> i8 {
    match shape {
        Shape::Button(label) => {
            disc(cell, 9.5, 8.0, 7.9);
            letters(cell, label, 9, 3);
            18
        }
        Shape::Key(label) => {
            key(cell, 1.0, 2.0, 19.0, 13.0);
            letters(cell, label, 10, 4);
            21
        }
        Shape::Stick(label) => {
            stick(cell, 8.5, 8.5, 7.4);
            small_letter(cell, label, 8, 5);
            17
        }
        Shape::StickClick(label) => {
            stick(cell, 9.5, 9.5, 6.4);
            small_letter(cell, label, 9, 6);
            press_arrow(cell, 9);
            17
        }
    }
}

struct Cell<'a> {
    texture: &'a mut Texture,
    u: usize,
    v: usize,
}
impl Cell<'_> {
    fn at(&self, x: usize, y: usize) -> usize { ((self.v + y) * self.texture.width as usize + self.u + x) * 4 }
    fn clear(&mut self) {
        for y in 0..CELL_H {
            for x in 0..CELL_W {
                let i = self.at(x, y);
                self.texture.rgba[i..i + 4].fill(0);
            }
        }
    }
    /// Source-over in straight colour, raw GS alpha.
    fn blend(&mut self, x: usize, y: usize, rgb: [f32; 3], cover: f32) {
        if x >= CELL_W || y >= CELL_H || cover <= 0.0 {
            return;
        }
        let i = self.at(x, y);
        let px = &mut self.texture.rgba[i..i + 4];
        let (sa, da) = (cover.min(1.0), px[3] as f32 / OPAQUE);
        let a = sa + da * (1.0 - sa);
        for c in 0..3 {
            px[c] = ((rgb[c] * sa + px[c] as f32 * da * (1.0 - sa)) / a).round().clamp(0.0, 255.0) as u8;
        }
        px[3] = (a * OPAQUE).round() as u8;
    }
    /// Fills the coverage of `inside` (4×4 samples per texel) with `shade(x, y)`.
    fn fill(&mut self, inside: impl Fn(f32, f32) -> bool, shade: impl Fn(f32, f32) -> [f32; 3]) {
        for y in 0..CELL_H {
            for x in 0..CELL_W {
                let mut n = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        n += inside(x as f32 + (sx as f32 + 0.5) / 4.0, y as f32 + (sy as f32 + 0.5) / 4.0) as u32;
                    }
                }
                self.blend(x, y, shade(x as f32 + 0.5, y as f32 + 0.5), n as f32 / 16.0);
            }
        }
    }
}

fn grey(v: f32) -> [f32; 3] { [v, v, v * 1.06] }

/// A face button: a dark rim, a body lit from above.
fn disc(cell: &mut Cell, cx: f32, cy: f32, r: f32) {
    let d = move |x: f32, y: f32| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    cell.fill(move |x, y| d(x, y) <= r, |_, _| grey(30.0));
    cell.fill(move |x, y| d(x, y) <= r - 1.2, move |_, y| grey(86.0 - (y - cy + r) / (2.0 * r) * 40.0));
}

/// A trigger / grip key: bevelled rectangle, light top-left edge.
fn key(cell: &mut Cell, x0: f32, y0: f32, w: f32, h: f32) {
    let rounded = move |x: f32, y: f32, inset: f32| {
        let (l, t, r, b) = (x0 + inset, y0 + inset, x0 + w - inset, y0 + h - inset);
        let k = 1.5;
        let dx = (l + k - x).max(x - (r - k)).max(0.0);
        let dy = (t + k - y).max(y - (b - k)).max(0.0);
        x >= l && x <= r && y >= t && y <= b && dx * dx + dy * dy <= k * k
    };
    cell.fill(move |x, y| rounded(x, y, 0.0), move |x, y| if x - x0 < 1.0 || y - y0 < 1.0 { grey(150.0) } else { grey(28.0) });
    cell.fill(move |x, y| rounded(x, y, 1.0), move |_, y| grey(66.0 - (y - y0) / h * 30.0));
}

/// A thumbstick: the dark base and the cap's lighter rim.
fn stick(cell: &mut Cell, cx: f32, cy: f32, r: f32) {
    let d = move |x: f32, y: f32| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    cell.fill(move |x, y| d(x, y) <= r, |_, _| grey(24.0));
    cell.fill(move |x, y| d(x, y) <= r - 1.0, |_, _| grey(118.0));
    cell.fill(move |x, y| d(x, y) <= r - 2.2, move |_, y| grey(70.0 - (y - cy + r) / (2.0 * r) * 34.0));
}

/// A downward arrow over the stick, centred on column `cx`.
fn press_arrow(cell: &mut Cell, cx: usize) {
    for (y, half) in [(0usize, 2usize), (1, 1), (2, 0)] {
        for x in cx - half..=cx + half {
            cell.blend(x, y, grey(235.0), 1.0);
        }
    }
}

fn ink(cell: &mut Cell, rows: &[&str], x0: usize, y0: usize) {
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            if c == b'#' {
                cell.blend(x0 + x, y0 + y, [238.0, 238.0, 244.0], 1.0);
            }
        }
    }
}

/// The label's bold letters, one texel apart, centred on column `cx` from row `y0`.
fn letters(cell: &mut Cell, label: &str, cx: usize, y0: usize) {
    let found: Vec<_> = label.chars().filter_map(|c| LETTERS.iter().find(|l| l.0 == c)).collect();
    let width = found.iter().map(|l| l.1).sum::<usize>() + found.len().saturating_sub(1);
    let mut x = cx + 1 - width.div_ceil(2);
    for (_, w, rows) in found {
        ink(cell, rows, x, y0);
        x += w + 1;
    }
}

fn small_letter(cell: &mut Cell, label: &str, cx: usize, y0: usize) {
    for c in label.chars() {
        if let Some((_, w, rows)) = SMALL_LETTERS.iter().find(|l| l.0 == c) {
            ink(cell, rows, cx + 1 - w.div_ceil(2), y0);
        }
    }
}
