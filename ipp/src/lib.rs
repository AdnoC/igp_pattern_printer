use image::{Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod row_builder;

// The "Outline" color. Default is this.
pub const SEPARATOR_COLOR: Rgb8 = Rgb8([32, 32, 32]);

pub fn rgb8_to_true(rgb: Rgb8) -> colored::Color {
    colored::Color::TrueColor {
        r: rgb.0[0],
        g: rgb.0[1],
        b: rgb.0[2],
    }
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Clone, Copy, Debug)]
pub struct Rgb8(pub [u8; 3]);
trait ToRgb8 {
    fn to_rgb8(self) -> Rgb8;
}
impl ToRgb8 for Rgb<u8> {
    fn to_rgb8(self) -> Rgb8 {
        Rgb8(self.0)
    }
}
impl Rgb8 {
    pub fn to_hex(&self) -> String {
        fn num_to_hex(num: u8) -> char {
            if num < 10 {
                return ('0' as u8 + num) as char;
            }
            let num = num - 10;
            return ('A' as u8 + num) as char;
        }
        let r1 = num_to_hex(self.0[0] / 16);
        let r2 = num_to_hex(self.0[0] % 16);
        let g1 = num_to_hex(self.0[1] / 16);
        let g2 = num_to_hex(self.0[1] % 16);
        let b1 = num_to_hex(self.0[2] / 16);
        let b2 = num_to_hex(self.0[2] % 16);
        format!("#{}{}{}{}{}{}", r1, r2, g1, g2, b1, b2)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ColorMap {
    full_names: HashMap<Rgb8, String>,
    short_char: HashMap<Rgb8, String>,
}

impl ColorMap {
    pub fn new() -> ColorMap {
        ColorMap {
            full_names: HashMap::new(),
            short_char: HashMap::new(),
        }
    }

    pub fn has(&self, color: Rgb8) -> bool {
        self.full_names.contains_key(&color)
    }

    pub fn add_entry(&mut self, color: Rgb8, entry: ColorEntry) {
        self.full_names.insert(color, entry.full_name);
        self.short_char.insert(color, entry.one_char);
    }

    pub fn full_name(&self, color: Rgb8) -> &str {
        &self.full_names[&color]
    }

    pub fn one_char(&self, color: Rgb8) -> &str {
        &self.short_char[&color]
    }
}

#[derive(Debug, Serialize)]
pub struct ColorEntry {
    pub full_name: String,
    pub one_char: String,
}


fn flood_fill(img: &mut RgbImage, (x, y): (u32, u32)) {
    if img[(x, y)].to_rgb8() == SEPARATOR_COLOR {
        return;
    }
    img[(x, y)] = Rgb(SEPARATOR_COLOR.0);

    if x > 0 {
        flood_fill(img, (x - 1, y));
    }
    if y > 0 {
        flood_fill(img, (x, y - 1));
    }
    if x + 1 < img.width() {
        flood_fill(img, (x + 1, y));
    }
    if y + 1 < img.height() {
        flood_fill(img, (x, y + 1));
    }
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Clone, Debug)]
pub struct Progress {
    row: usize,
    col: usize,
}
impl Progress {
    pub fn new() -> Self {
        Progress { row: 2, col: 1 }
    }
    fn reset(&mut self) {
        self.row = 2;
        self.col = 1;
    }
}

#[derive(Clone, Copy)]
pub enum NextPreview {
    Pixel(Option<Rgb8>),
    Tri([Option<Rgb8>; 3])
}
pub struct App {
    pub lines: Vec<Vec<Rgb8>>,
    pub rows: Vec<Vec<Rgb8>>,
    pub current_pixel: NextPreview,
    pub next_pixel: NextPreview,
    pub ensure_current_on_screen: bool,
    pub progress: Progress,
}
impl App {
    pub fn initialize_lines(rows: &Vec<Vec<Rgb8>>, progress: &Progress) -> Vec<Vec<Rgb8>> {
        if progress.row < 3 {
            vec![
                rows[0].iter().take(progress.col + 1).cloned().collect(),
                rows[1].iter().take(progress.col).cloned().collect(),
                rows[2].iter().take(progress.col + 1).cloned().collect(),
            ]

        } else {
            let mut lines: Vec<Vec<Rgb8>> = rows.iter().take(progress.row).cloned().collect();
            lines.push(
                rows[progress.row - 1]
                    .iter()
                    .take(progress.col + 1)
                    .cloned()
                    .collect(),
            );
            lines
        }
    }

    pub fn new(rows: Vec<Vec<Rgb8>>, progress: Progress) -> App {
        use NextPreview::*;
        let lines = App::initialize_lines(&rows, &progress);
        let next_pixel = if progress.row >= 3 {
            Pixel(rows[progress.row].get(progress.col).copied())
        } else {
            Tri([
                rows[0].get(progress.col + 1).copied(),
                rows[1].get(progress.col).copied(),
                rows[2].get(progress.col + 1).copied(),
            ])
        };
        let current_pixel = if progress.row >= 3 {
            Pixel(rows[progress.row].get(progress.col - 1).copied())
        } else {
            Tri([
                rows[0].get(progress.col).copied(),
                rows[1].get(progress.col - 1).copied(),
                rows[2].get(progress.col).copied(),
            ])
        };
        App {
            ensure_current_on_screen: false,
            lines,
            rows,
            current_pixel,
            next_pixel,
            progress,
        }

    }
}

// Lifecycle methods
impl App {
    pub fn tick(&mut self) {
        self.ensure_current_on_screen = true;
        self.progress.col += 1;
        self.current_pixel = self.next_pixel;
        if self.is_done_with_line() {
            self.progress.row += 1;
            self.progress.col = 0;
            self.lines.push(vec![]);
            self.current_pixel = NextPreview::Pixel(self.rows.get(self.progress.row).and_then(|row| row.get(0).copied()));
        }
        if self.progress.row < 3 {
            self.rows[0].get(self.lines[0].len()).map(|val| self.lines[0].push(*val));
            self.rows[1].get(self.lines[1].len()).map(|val| self.lines[1].push(*val));
            self.rows[2].get(self.lines[2].len()).map(|val| self.lines[2].push(*val));
        } else {
            if let Some(line) = self.lines.last_mut() {
                self.rows[self.progress.row]
                     .get(line.len())
                     .map(|val| line.push(*val));
            }
        }

        self.next_pixel = if self.progress.row >= 3 {
            NextPreview::Pixel(self.rows[self.progress.row].get(self.progress.col).copied())
        } else {
            NextPreview::Tri([
                self.rows[0].get(self.progress.col + 1).copied(),
                self.rows[1].get(self.progress.col).copied(),
                self.rows[2].get(self.progress.col + 1).copied(),
            ])
        };
    }

    pub fn reset(&mut self) {
        self.progress.reset();
        self.lines = App::initialize_lines(&self.rows, &self.progress);

    }

    pub fn is_done(&self) -> bool {
        self.progress.row >= (self.rows.len() - 1)
            && self.progress.col >= self.rows.last().map(|r| r.len()).unwrap_or(1) - 1
    }

    pub fn is_done_with_line(&self) -> bool {
        if self.progress.row < 3 {
            let max_len = self.rows[0].len().max(self.rows[1].len()).max(self.rows[2].len());
            self.progress.col >= max_len
        } else {
            self.progress.col >= self.rows[self.progress.row].len()
        }
    }
}
