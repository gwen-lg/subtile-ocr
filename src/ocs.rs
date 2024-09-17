use image::{GrayImage, Luma};
use subtile::content::{Area, AreaValues};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("The image is not correctly prepared, some pixels are not white or black")]
    ImageWithGrayIsInvalid,

    #[error("No character found")]
    NoCharactersFound,
}

#[derive(Debug, Clone)]
struct Piece {
    area: Area,
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
            pixels,
            img: None,
        }
    }

    pub fn area(&self) -> Area {
        self.area
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
        let mut new_pieces: Vec<Piece> = Vec::with_capacity(self.pieces.len());
        self.pieces.drain(0..self.pieces.len()).for_each(|piece| {
            if let Some(last_piece) = new_pieces.last_mut() {
                if last_piece.area().contains_x(piece.area()) {
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
        let pieces = Self::split_in_pieces(self.img)?;
        if pieces.is_empty() {
            return Err(Error::NoCharactersFound);
        }

        // Compute lines from pieces
        let mut lines = Self::organize_pieces_in_lines(pieces);

        // sort pieces in lines by left coordinate. Need to be configurable to manage languages with right to left order.
        lines.iter_mut().for_each(|line| line.sort_pieces());

        // group accent piece with base glyph
        lines.iter_mut().for_each(|line| line.group_accent());

        lines
            .iter_mut()
            .for_each(|line| line.pieces.iter_mut().for_each(|piece| piece.create_img()));

        Ok(ImagePieces { lines })
    }

    // Split the image into part of adjacent pixels
    fn split_in_pieces(mut image: GrayImage) -> Result<Vec<Piece>, Error> {
        let mut pieces = Vec::with_capacity(128);
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

        Ok(pieces)
    }

    // Organize the pieces in lines
    fn organize_pieces_in_lines(mut pieces: Vec<Piece>) -> Vec<Line> {
        let mut lines: Vec<Line> = Vec::with_capacity(2);
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
        lines
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
