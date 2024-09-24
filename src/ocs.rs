use image::{GrayImage, Luma};
use subtile::content::{Area, AreaValues};
use thiserror::Error;

use crate::glyph::{Glyph, GlyphLibrary};

#[derive(Debug, Error)]
pub enum Error {
    #[error("The image is not correctly prepared, some pixels are not white or black")]
    ImageWithGrayIsInvalid,

    #[error("No character found")]
    NoCharactersFound,
}

///TODO move
pub trait GlyphCharAsker {
    /// TODO
    fn ask_char_for_glyph(&self, piece: &Piece) -> String;
}

#[derive(Debug, Clone)]
pub struct Piece {
    area: Area,
    // x: u32,
    // y: u32,
    // width: u32,
    // height: u32,
    /// list of pixels of the letter
    pixels: Vec<(u32, u32)>,
    img: Option<GrayImage>,
}

impl Piece {
    pub fn new(pixels: Vec<(u32, u32)>) -> Self {
        let x1 = pixels.iter().map(|(x, _)| *x).reduce(u32::min).unwrap();
        let y1 = pixels.iter().map(|(_, y)| *y).reduce(u32::min).unwrap();
        let x2 = pixels.iter().map(|(x, _)| *x).reduce(u32::max).unwrap();
        let y2 = pixels.iter().map(|(_, y)| *y).reduce(u32::max).unwrap();

        let area = Area::try_from(AreaValues {
            x1: x1.try_into().unwrap(),
            y1: y1.try_into().unwrap(),
            x2: x2.try_into().unwrap(),
            y2: y2.try_into().unwrap(),
        })
        .unwrap();

        Self {
            area,
            // x,
            // y,
            // width: x_max - x,
            // height: y_max - y,
            pixels,
            img: None,
        }
    }

    pub fn area(&self) -> Area {
        self.area
    }
    pub fn img(&self) -> &GrayImage {
        self.img.as_ref().unwrap()
    }
    // pub fn point_min(&self) -> (u16, u16) {
    //     (self.area.left(), self.area.top())
    // }
    // pub fn point_max(&self) -> (u32, u32) {
    //     (self.x + self.width, self.y + self.height)
    // }
    pub fn pixels(&self) -> &[(u32, u32)] {
        &self.pixels
    }
    pub fn create_img(&mut self) {
        assert!(self.img.is_none());

        let img = GrayImage::from_fn(
            u32::from(self.area.width()),
            u32::from(self.area.height()),
            |x, y| {
                let x = x + u32::from(self.area.left());
                let y = y + u32::from(self.area.top());
                if self.pixels.contains(&(x, y)) {
                    COLOR_BLACK
                } else {
                    COLOR_WHITE
                }
            },
        );
        self.img = Some(img);
    }
    pub fn extend(&mut self, mut other: Self) {
        assert!(self.img.is_none()); // should be processed before img creation
        assert!(self.area.intersect_x(other.area));

        self.area.extend(other.area);
        self.pixels.append(&mut other.pixels);
    }
}

/// Line of pieces
pub struct Line {
    area: Area,
    pieces: Vec<Piece>,
}

impl Line {
    pub fn from_piece(piece: Piece) -> Self {
        Self {
            area: piece.area(),
            pieces: vec![piece],
        }
    }
    pub fn extend_with_piece(&mut self, piece: Piece) {
        self.area.extend(piece.area());
        self.pieces.push(piece);
    }
    pub fn sort_pieces(&mut self) {
        self.pieces.sort_by_key(|piece| piece.area().left());
    }
    pub fn group_accent(&mut self) {
        //TODO: don't manage correctly all case, example with 'ï'
        let mut new_pieces: Vec<Piece> = Vec::new();
        self.pieces.drain(0..self.pieces.len()).for_each(|piece| {
            if let Some(last_piece) = new_pieces.last_mut() {
                if last_piece.area().intersect_x(piece.area()) {
                    last_piece.extend(piece);
                } else {
                    new_pieces.push(piece);
                }
            } else {
                new_pieces.push(piece);
            }
        });

        self.pieces = new_pieces;
    }

    pub fn pieces(&self) -> impl Iterator<Item = &Piece> {
        self.pieces.iter()
    }
}

/// Result of a split
pub struct ImagePieces {
    lines: Vec<Line>,
}

impl ImagePieces {
    /// return a ref on slice
    pub fn images(&self) -> impl Iterator<Item = impl Iterator<Item = &GrayImage>> {
        self.lines
            .iter()
            .map(|line| line.pieces.iter().map(|piece| piece.img.as_ref().unwrap()))
    }

    /// Tried to recognize text of the image
    pub fn process_to_text(
        &self,
        glyph_lib: &mut GlyphLibrary,
        asker: &impl GlyphCharAsker,
    ) -> String {
        // test to get character for glyph
        let mut text = String::new();
        self.lines.iter().for_each(|line| {
            line.pieces().for_each(|piece| {
                let character = glyph_lib.find(piece.img());
                if let Some(character) = character {
                    text.push_str(character);
                } else {
                    let proximities = glyph_lib.find_closest(piece.img());
                    // proximities.iter().for_each(|(sum, glyph)| {
                    //     let nb_pixels = piece.len();
                    //     let proximity = *sum as f32 / nb_pixels as f32;
                    //     println!(
                    //         "{:?} : {}/{} => {}",
                    //         glyph.chars(),
                    //         sum,
                    //         nb_pixels,
                    //         proximity
                    //     );
                    // });
                    let ok = if let Some((sum, closest_glyph)) = proximities.first() {
                        let nb_pixels = piece.img().len();
                        let proximity = *sum as f32 / nb_pixels as f32;
                        if proximity >= 0.95 {
                            if let Some(character) = closest_glyph.chars() {
                                text.push_str(character);
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if !ok {
                        let characters = asker.ask_char_for_glyph(piece);
                        text.push_str(characters.as_str());
                        glyph_lib.add_glyph(Glyph::new(piece.img().clone(), Some(characters)));
                    }
                    // TODO: handle space
                }
            });
            text.push('\n');
        });

        text
    }
}

/// A struct to extract character from an image (black and white)
pub struct ImageCharacterSplitter {
    img: GrayImage,
}

const COLOR_BLACK: Luma<u8> = Luma([0]);
const COLOR_WHITE: Luma<u8> = Luma([255]);
impl ImageCharacterSplitter {
    /// Create a `CharacterSplitter` for provided image.
    pub fn from_image(image: &GrayImage) -> Self {
        Self { img: image.clone() }
    }

    /// Split image into a list of character image
    pub fn split_in_character_img(self) -> Result<ImagePieces, Error> {
        let mut image = self.img;

        let mut pieces = vec![];

        let (width, height) = image.dimensions();
        (0..height).try_for_each(|y| {
            (0..width).try_for_each(|x| {
                let pixel_color = image.get_pixel(x, y);
                if *pixel_color == COLOR_BLACK {
                    let new_piece = cut_piece(x, y, &mut image);
                    pieces.push(new_piece);
                } else if *pixel_color == COLOR_WHITE {
                    // just ignore white
                } else {
                    return Err(Error::ImageWithGrayIsInvalid);
                }
                Ok(())
            })
        })?;

        if pieces.is_empty() {
            return Err(Error::NoCharactersFound);
        }

        let mut lines: Vec<Line> = Vec::new();
        pieces.drain(..).for_each(|piece| {
            if let Some(line) = lines
                .iter_mut()
                .find(|Line { area, .. }| area.intersect_y(piece.area()))
            {
                line.extend_with_piece(piece);
            } else {
                lines.push(Line::from_piece(piece));
            }
        });
        //TODO: manage line with only accents

        // sort pieces in lines by left coordinate. Need to be configurable to manage languages with right to left order.
        lines.iter_mut().for_each(|line| line.sort_pieces());

        // group accent piece with base glyph
        lines.iter_mut().for_each(|line| line.group_accent());

        lines
            .iter_mut()
            .for_each(|line| line.pieces.iter_mut().for_each(|piece| piece.create_img()));

        Ok(ImagePieces { lines })
    }
}

fn cut_piece(x: u32, y: u32, image: &mut GrayImage) -> Piece {
    let (image_width, image_height) = image.dimensions();
    let mut piece_pixels = vec![(x, y)];
    let mut cur_pix_idx = 0;

    while cur_pix_idx < piece_pixels.len() {
        let (x, y) = piece_pixels[cur_pix_idx];

        // non-diagonal adjacent pixels
        let mut adjacent_pixels = Vec::with_capacity(4); //TODO: array vec
        if x > 0 {
            adjacent_pixels.push((x - 1, y));
        }
        if x < (image_width - 1) {
            adjacent_pixels.push((x + 1, y));
        }
        if y > 0 {
            adjacent_pixels.push((x, y - 1));
        }
        if y < (image_height - 1) {
            adjacent_pixels.push((x, y + 1));
        }

        adjacent_pixels.into_iter().for_each(|(x, y)| {
            if *image.get_pixel(x, y) == COLOR_BLACK {
                piece_pixels.push((x, y));
                *image.get_pixel_mut(x, y) = COLOR_WHITE;
            }
        });

        cur_pix_idx += 1;
    }

    Piece::new(piece_pixels)
}
