use image::{GrayImage, Luma};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("The image is not correctly prepared, some pixels are not white or black")]
    ImageWithGrayIsInvalid,
}

/// Result of a split
pub struct ImagePieces {
    pieces: Vec<GrayImage>,
}

impl ImagePieces {
    /// return a ref on slice
    pub fn images(&self) -> &[GrayImage] {
        self.pieces.as_slice()
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

        Ok(ImagePieces { pieces })
    }

    // Split the image into part of adjacent pixels
    fn split_in_pieces(mut image: GrayImage) -> Result<Vec<GrayImage>, Error> {
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
}

fn cut_piece(x: u32, y: u32, image: &mut GrayImage) -> GrayImage {
    let (image_width, image_height) = image.dimensions();
    let mut piece_pixels = vec![(x, y)];
    let mut cur_pix_idx = 0;

    while cur_pix_idx < piece_pixels.len() {
        let (x, y) = piece_pixels[cur_pix_idx];
        if x < (image_width - 1) && *image.get_pixel(x + 1, y) == COLOR_BLACK {
            piece_pixels.push((x + 1, y));
            *image.get_pixel_mut(x + 1, y) = COLOR_WHITE;
        }
        if x > 0 && *image.get_pixel(x - 1, y) == COLOR_BLACK {
            piece_pixels.push((x - 1, y));
            *image.get_pixel_mut(x - 1, y) = COLOR_WHITE;
        }
        if y > 0 && *image.get_pixel(x, y - 1) == COLOR_BLACK {
            piece_pixels.push((x, y - 1));
            *image.get_pixel_mut(x, y - 1) = COLOR_WHITE;
        }
        if y < (image_height - 1) && *image.get_pixel(x, y + 1) == COLOR_BLACK {
            piece_pixels.push((x, y + 1));
            *image.get_pixel_mut(x, y + 1) = COLOR_WHITE;
        }

        cur_pix_idx += 1;
    }

    let x_min = piece_pixels
        .iter()
        .map(|(x, _)| *x)
        .reduce(u32::min)
        .unwrap();
    let x_max = piece_pixels
        .iter()
        .map(|(x, _)| *x)
        .reduce(u32::max)
        .unwrap();
    let y_min = piece_pixels
        .iter()
        .map(|(_, y)| *y)
        .reduce(u32::min)
        .unwrap();
    let y_max = piece_pixels
        .iter()
        .map(|(_, y)| *y)
        .reduce(u32::max)
        .unwrap();
    let width = x_max - x_min;
    let height = y_max - y_min;

    GrayImage::from_fn(width, height, |x, y| {
        let x = x + x_min;
        let y = y + y_min;
        if piece_pixels.contains(&(x, y)) {
            COLOR_BLACK
        } else {
            COLOR_WHITE
        }
    })
}
