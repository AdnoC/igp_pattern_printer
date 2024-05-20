use std::{
    io,
    error::Error,
    collections::HashMap,
};
use image::{
    io::Reader as ImageReader,
    Rgb,
    RgbImage,
};
use colored::Colorize;

type Rgb8 = Rgb<u8>;

// The "Outline" color. Default is this.
const SEPARATOR_COLOR: Rgb8 = Rgb([32, 32, 32]);

fn rgb8_to_true(rgb: Rgb8) -> colored::Color {
    colored::Color::TrueColor {
        r: rgb.0[0],
        g: rgb.0[1],
        b: rgb.0[2],
    }
}

struct ColorMap {
    color_map: HashMap<Rgb8, String>,
}

impl ColorMap {
    fn new() -> ColorMap {
        ColorMap {
            color_map: HashMap::new(),
        }
    }

    fn ensure_mapped(&mut self, color: Rgb8) -> Result<(), Box<dyn Error>> {
        use io::Write;

        if self.color_map.contains_key(&color) {
            return Ok(())
        }
        let colored_rgb = format!("{:?}", color)
            .color(rgb8_to_true(color))
            .on_color(rgb8_to_true(SEPARATOR_COLOR));
        println!("Found new color: {}", colored_rgb);
        print!("Please give it a name:");
        io::stdout().flush()?;
        let mut name = String::new();
        io::stdin().read_line(&mut name)?;
        self.color_map.insert(color, name.trim().to_owned());
        Ok(())
    }

    fn map(&self, color: Rgb8) -> &str {
        &self.color_map[&color]
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args();
    args.next();
    let file = match args.next() {
        Some(f) => f,
        None => return Err("File argument required.".into()),
    };
    println!("Opening file {}", file);

    let img = ImageReader::open(file)?.decode()?.to_rgb8();
    let mut color_map = ColorMap::new();

    let rows = build_rows(img, &mut color_map)?;

    let (_, term_width) = termion.terminal_size()?;
    for (row_idx, row) in rows.into_iter().enumerate() {
        if row_idx % 2 == 1 {
            print!(" ");
        }
        for p in row {
            let colored_p = color_map.map(p)
                .color(rgb8_to_true(p))
                .on_color(rgb8_to_true(SEPARATOR_COLOR));
            print!("{} ", colored_p);
        }
        println!();
    }
    Ok(())
}

fn build_rows(mut img: RgbImage, color_map: &mut ColorMap) -> Result<Vec<Vec<Rgb8>>, Box<dyn Error>> {
    let mut rows: Vec<Vec<Rgb8>> = vec![];
    let mut current_row: Vec<Rgb8> = vec![];
    for y in 0..(img.height()) {
        for x in 0..(img.width()) {
            if img[(x, y)] == SEPARATOR_COLOR {
                continue;
            }
            current_row.push(img[(x, y)]);
            color_map.ensure_mapped(img[(x, y)])?;
            flood_fill(&mut img, (x, y));
        }
        if !current_row.is_empty() {
            rows.push(current_row);
            current_row = vec![];
        }
    }
    Ok(rows)
}

fn flood_fill(img: &mut RgbImage, (x, y): (u32, u32)) {
    if img[(x, y)] == SEPARATOR_COLOR {
        return
    }
    img[(x, y)] = SEPARATOR_COLOR;

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
