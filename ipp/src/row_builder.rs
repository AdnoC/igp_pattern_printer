use crate::{flood_fill, ColorEntry, ColorMap, Rgb8, ToRgb8, SEPARATOR_COLOR};
use image::RgbImage;

#[derive(Debug)]
pub struct RowBuilder {
    img: RgbImage,
    rows: Vec<Vec<Rgb8>>,
    current_row: Vec<Rgb8>,
    x: u32,
    y: u32,
}

impl RowBuilder {
    pub fn new(img: RgbImage) -> RowBuilder {
        RowBuilder {
            img,
            rows: vec![],
            current_row: vec![],
            x: 0,
            y: 0,
        }
    }

    pub fn build(&mut self, color_map: &mut ColorMap) -> BuildState {
        for y in (self.y)..(self.img.height()) {
            'row: for x in (self.x)..(self.img.width()) {
                self.x = x;
                self.y = y;
                let pixel = self.img[(x, y)].to_rgb8();
                if pixel == SEPARATOR_COLOR {
                    continue 'row;
                }
                if !color_map.has(pixel) {
                    return BuildState::NewColor(pixel);
                }
                self.current_row.push(pixel);
                flood_fill(&mut self.img, (x, y));
            }

            if !self.current_row.is_empty() {
                let current = std::mem::replace(&mut self.current_row, vec![]);
                self.rows.push(current);
            }
            self.x = 0;
        }
        BuildState::Complete(self.rows.clone())
    }

    pub fn continue_build(&mut self, entry: ColorEntry, color_map: &mut ColorMap) -> BuildState {
        let initial_pixel = self.img[(self.x, self.y)].to_rgb8();
        color_map.add_entry(initial_pixel, entry);
        self.build(color_map)
    }
}

pub enum BuildState {
    Complete(Vec<Vec<Rgb8>>),
    NewColor(Rgb8),
}
