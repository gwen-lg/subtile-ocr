use derive_more::derive::AsRef;
use image::{GrayImage, Luma};
use ron::ser::PrettyConfig;
use serde::{
    de::{self, Visitor},
    ser::{self, SerializeSeq},
    Deserialize, Serialize,
};
use std::{
    fmt,
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::PathBuf,
};
use thiserror::Error;

/// Errors of the glyph
#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid pixel value `{0}` for serialization")]
    PixelSerializeInvalidValue(u8),

    #[error("Invalid pixel value `{0}` for deserialization")]
    PixelsDeserializeInvalidValue(char),

    #[error("Failed to serialize a Glyph with ron format")]
    GlyphRonSerialization(#[source] ron::Error),

    #[error("Failed to create directory for save Glyphs Library")]
    GlyphsLibraryCreateDirectory(#[source] io::Error),

    #[error("Failed to open Glyphs Library file to write it")]
    GlyphsLibraryOpenFile(#[source] io::Error),
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

impl GlyphImage {
    fn pix_to_char((_, _, pix): (u32, u32, &Luma<u8>)) -> Result<char, Error> {
        Self::luma_to_char(*pix)
    }
    fn luma_to_char(pix: Luma<u8>) -> Result<char, Error> {
        match pix.0 {
            [0] => Ok('8'),
            [255] => Ok(' '),
            [val] => Err(Error::PixelSerializeInvalidValue(val)),
        }
    }
    fn char_to_pix(pix: char) -> Result<u8, Error> {
        match pix {
            '8' => Ok(0u8),
            ' ' => Ok(255u8),
            c => Err(Error::PixelsDeserializeInvalidValue(c)),
        }
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
                let pixel_str = pixels
                    .map(Self::pix_to_char)
                    .collect::<Result<String, _>>()
                    .map_err(ser::Error::custom)?;

                seq.serialize_element(pixel_str.as_str())
            })?;
            seq.end()
        } else {
            //TODO: serialize glyph size
            let pixels = self.0.enumerate_pixels();
            let pixel_str = pixels
                .map(Self::pix_to_char)
                .collect::<Result<String, _>>()
                .map_err(ser::Error::custom)?;
            serializer.serialize_str(pixel_str.as_str())
        }
    }
}

struct GlyphImageHumanVisitor;
impl<'de> Visitor<'de> for GlyphImageHumanVisitor {
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
                    row_pixels
                        .chars()
                        .map(|p| GlyphImage::char_to_pix(p).map_err(de::Error::custom))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let image = GrayImage::from_vec(width as u32, height as u32, pixels).unwrap();
            Ok(image.into())
        } else {
            Err(<A::Error as serde::de::Error>::custom("Empty glyph image"))
        }
    }
}

impl<'de> Deserialize<'de> for GlyphImage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        assert!(deserializer.is_human_readable()); //TODO: handle no human readable
        deserializer.deserialize_seq(GlyphImageHumanVisitor)
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

// Define the `filename` of the library .
const LIBRARY_FILENAME: &str = "glyph_library.ron";

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

    /// Save the Library into a `path`.
    /// Note: The folder shouldn't have other data, otherwise the data could be erased.
    pub fn save_to_path(&self, path: impl Into<PathBuf>) -> Result<(), Error> {
        let path = path.into();
        fs::create_dir_all(path.as_path()).map_err(Error::GlyphsLibraryCreateDirectory)?;

        let mut glyph_library_filename = path;
        glyph_library_filename.push(LIBRARY_FILENAME);
        let file = File::create(glyph_library_filename).map_err(Error::GlyphsLibraryOpenFile)?;
        let writer = BufWriter::new(file);
        self.save(writer)
    }

    /// Serialize Library to backup data.
    pub fn save(&self, writer: impl Write) -> Result<(), Error> {
        ron::ser::to_writer_pretty(writer, &self.glyphs, PrettyConfig::default())
            .map_err(Error::GlyphRonSerialization)
    }
}
