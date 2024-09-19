use derive_more::derive::AsRef;
use image::GrayImage;
use thiserror::Error;

/// Errors of the glyph
#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to serialize a Glyph with ron format")]
    GlyphRonSerialization(#[source] ron::Error),
}

/// Struct wrapper for `GlyphImage`
/// Allow to implement `Serialize` and `Deserialize` from serialimage
#[derive(AsRef, Debug, Eq, PartialEq)]
#[repr(transparent)]
struct GlyphImage(GrayImage);
impl From<GrayImage> for GlyphImage {
    fn from(img: GrayImage) -> Self {
        Self(img)
    }
}

/// struct to
#[derive(Debug, PartialEq, Eq)]
pub struct Glyph {
    img: GlyphImage,
    characters: Option<String>,
}

impl Glyph {
    pub fn new(img: GrayImage, characters: Option<String>) -> Self {
        Self {
            img: img.into(),
            characters,
        }
    }

    pub fn chars(&self) -> Option<&String> {
        self.characters.as_ref()
    }
}

/// Manage a library of glyph.
pub struct GlyphLibrary {
    glyphs: Vec<Glyph>,
}

impl GlyphLibrary {
    pub fn new() -> Self {
        Self { glyphs: Vec::new() }
    }

    pub fn find(&self, glyph_img: &GrayImage) -> Option<&str> {
        self.glyphs
            .iter()
            .find(|glyph| {
                glyph_img.dimensions() == glyph.img.0.dimensions()
                    && glyph_img.as_raw() == glyph.img.0.as_raw()
            })
            .and_then(|glyph| glyph.characters.as_deref())
    }

    //TODO: weight according to if the pixel witch is different is on an edge
    // and or if the different pixels are closed or scattered
    pub fn find_closest(&self, glyph_img: &GrayImage) -> Vec<(i32, &Glyph)> {
        //let count = glyph_img.len();
        let mut glyphs_proximity = self
            .glyphs
            .iter()
            .filter(|glyph| glyph_img.dimensions() == glyph.img.0.dimensions())
            .map(|glyph| {
                let sum = glyph
                    .img
                    .0
                    .iter()
                    .zip(glyph_img.iter())
                    .fold(0, |sum, (a, b)| {
                        sum + {
                            if a == b {
                                1
                            } else {
                                0
                            }
                        }
                    });
                (sum, glyph)
            })
            .collect::<Vec<_>>();
        glyphs_proximity.sort_by(|(a_sum, _), (b_sum, _)| b_sum.cmp(a_sum));
        glyphs_proximity
    }

    /// Add a glyph in Library
    pub fn add_glyph(&mut self, glyph: Glyph) {
        self.glyphs.push(glyph);
    }
}
