use image::RgbImage;
use crate::{
    ColorMap,
    ColorEntry,
    flood_fill,
    Rgb8,
    ToRgb8,
    SEPARATOR_COLOR,
};

pub struct RowBuilder<'a> {
    img: RgbImage,
    color_map: &'a mut ColorMap,
    rows: Vec<Vec<Rgb8>>,
    current_row: Vec<Rgb8>,
    x: u32,
    y: u32,
}

impl<'a> RowBuilder<'a> {
    pub fn new(
        img: RgbImage,
        color_map: &mut ColorMap,
    ) -> RowBuilder {
        RowBuilder {
            img,
            color_map,
            rows: vec![],
            current_row: vec![],
            x: 0,
            y: 0,
        }
    }

    pub fn build(&mut self) -> BuildState {
        for y in (self.y)..(self.img.height()) {
            for x in (self.x)..(self.img.width()) {
                self.x = x;
                self.y = y;
                let pixel = self.img[(x, y)].to_rgb8();
                if pixel == SEPARATOR_COLOR {
                    continue;
                }
                 if pixel == Rgb8([0, 0, 0]) { continue;} println!("x, y, p, p2: {:?}", (x, y, self.img[(x, y)], pixel));
                if !self.color_map.has(pixel) {
                    return BuildState::NewColor(pixel)
                }
                self.current_row.push(pixel);
                //flood_fill(&mut self.img, (x, y));
            }
            if !self.current_row.is_empty() {
                let current = std::mem::replace(&mut self.current_row, vec![]);
                self.rows.push(current);
            }
        }
        BuildState::Complete(self.rows.clone())
    }

    pub fn continue_build(&mut self, entry: ColorEntry) -> BuildState {
        let initial_pixel = self.img[(self.x, self.y)].to_rgb8();
        self.color_map.add_entry(initial_pixel, entry);
        self.build()
    }

}

pub enum BuildState {
    Complete(Vec<Vec<Rgb8>>),
    NewColor(Rgb8)
}
