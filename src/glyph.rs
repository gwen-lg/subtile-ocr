use derive_more::derive::AsRef;
use image::{GrayImage, Luma};
use ron::ser::PrettyConfig;
use serde::{de::Visitor, ser::SerializeSeq, Deserialize, Serialize};
use std::{
    fmt,
    io::{Read, Write},
};
use thiserror::Error;

/// Errors of the glyph
#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to serialize a Glyph with ron format")]
    GlyphRonSerialization(#[source] ron::Error),

    #[error("Failed to deserialize a Glyph with ron format")]
    GlyphRonDeserialization(#[source] ron::de::SpannedError),
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
impl Serialize for GlyphImage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            let mut rows = self.0.enumerate_rows();
            let mut seq = serializer.serialize_seq(Some(rows.len()))?;
            rows.try_for_each(|(_idx, pixels)| {
                let pixel_str = pixels.map(Self::pix_to_char).collect::<String>();

                seq.serialize_element(pixel_str.as_str())
            })?;
            seq.end()
        } else {
            //TODO: serialize glyph size
            let pixels = self.0.enumerate_pixels();
            let pixel_str = pixels.map(Self::pix_to_char).collect::<String>();
            serializer.serialize_str(pixel_str.as_str())
        }
    }
}

struct GlyphImageVisitor;
impl<'de> Visitor<'de> for GlyphImageVisitor {
    type Value = GlyphImage;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an array of pixel with character ' ' for white and '8' for black")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut rows: Vec<String> = Vec::new();
        while let Some(elem) = seq.next_element()? {
            rows.push(elem);
        }

        let height = rows.len();
        if height > 0 {
            let width = rows[0].len();
            //let pixels: Vec<u8> = Vec::with_capacity(width * height);
            let pixels = rows
                .iter()
                .flat_map(|row_pixels| {
                    row_pixels.chars().map(|pixel_str| match pixel_str {
                        '8' => 0u8,
                        ' ' => 255u8,
                        _ => 125u8, //Todo, manage error
                    })
                })
                .collect::<Vec<_>>();
            let image = GrayImage::from_vec(width as u32, height as u32, pixels).unwrap();
            Ok(image.into())
        } else {
            Err(<A::Error as serde::de::Error>::custom("Empty glyph image"))
        }

        //TODO: handle no human readable
    }
    // fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    // where
    //     E: serde::de::Error,
    // {
    //     v
    // }
}

impl<'de> Deserialize<'de> for GlyphImage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        assert!(deserializer.is_human_readable());
        let visitor = GlyphImageVisitor {};
        // let width = 10; //TODO
        // let height = 10; //TODO
        deserializer.deserialize_seq(visitor)
        //todo!();
        // let glyph_pixel = vec![];
        // let img = GrayImage::from_vec(width, height, glyph_pixel).unwrap();
        //Ok(Self(img.try_into().unwrap())) // TODO: remove unwrap
    }
}

impl GlyphImage {
    fn pix_to_char((_, _, pix): (u32, u32, &Luma<u8>)) -> char {
        Self::luma_to_char(*pix)
    }
    fn luma_to_char(pix: Luma<u8>) -> char {
        match pix.0 {
            [0] => '8',
            [255] => ' ',
            _ => '?', //TODO: manage error
        }
    }
}

/// struct to
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
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

// /// Manage a library of glyph.
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

    /// TODO
    pub fn add_glyph(&mut self, glyph: Glyph) {
        self.glyphs.push(glyph);
    }

    /// TODO
    pub fn load(&mut self, reader: impl Read) -> Result<(), Error> {
        self.glyphs = ron::de::from_reader(reader).map_err(Error::GlyphRonDeserialization)?;
        Ok(())
    }

    /// TODO
    pub fn save(&self, writer: impl Write) -> Result<(), Error> {
        ron::ser::to_writer_pretty(writer, &self.glyphs, PrettyConfig::default())
            .map_err(Error::GlyphRonSerialization)
    }
}
