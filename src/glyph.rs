use compact_str::CompactString;
use derive_more::derive::AsRef;
use image::{GrayImage, Luma};
use ron::ser::PrettyConfig;
use serde::{
    de::{self, Visitor},
    ser::{self, SerializeSeq, SerializeStruct},
    Deserialize, Serialize,
};
use std::{
    fmt,
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Write},
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

    #[error("Failed to deserialize a Glyph with ron format")]
    GlyphRonDeserialization(#[source] ron::de::SpannedError),

    #[error("There is no Glyph Library to load.")]
    NoFileToLoad(#[source] io::Error),

    #[error("Failed to load Glyph Library")]
    FailedToLoadFile(#[source] io::Error),

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
                    .collect::<Result<CompactString, _>>()
                    .map_err(ser::Error::custom)?;

                seq.serialize_element(pixel_str.as_str())
            })?;
            seq.end()
        } else {
            let mut state = serializer.serialize_struct("GlyphImage", 3)?;
            state.serialize_field("s", &self.0.dimensions())?;
            let pixels = self.0.enumerate_pixels();
            //TODO: compact even more pixels with pack 8 pixels in a char
            let pixel_str = pixels
                .map(Self::pix_to_char)
                .collect::<Result<CompactString, _>>()
                .map_err(ser::Error::custom)?;
            state.serialize_field("p", pixel_str.as_str())?;
            state.end()
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
        let mut rows: Vec<CompactString> = Vec::with_capacity(16);
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
        if deserializer.is_human_readable() {
            deserializer.deserialize_seq(GlyphImageHumanVisitor)
        } else {
            #[derive(Deserialize)]
            #[serde(field_identifier, rename_all = "lowercase")]
            enum Field {
                S, // Size
                P, // Pixels
            }

            struct GlyphImageVisitor;
            impl<'de> Visitor<'de> for GlyphImageVisitor {
                type Value = GlyphImage;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("struct GlyphImage")
                }

                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: de::SeqAccess<'de>,
                {
                    let (width, height): (u32, u32) = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                    let pixels_str: String = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                    //TODO: compact even more pixels with pack 8 pixels in a char
                    let pixels = pixels_str
                        .chars()
                        .map(GlyphImage::char_to_pix)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(de::Error::custom)?;
                    let img = GrayImage::from_vec(width, height, pixels)
                        .ok_or_else(|| de::Error::custom("Failed to create Image for Glyph"))?;
                    Ok(GlyphImage(img))
                }
            }

            const FIELDS: &[&str] = &["s", "p"];
            deserializer.deserialize_struct("GlyphImage", FIELDS, GlyphImageVisitor)
        }
    }
}

/// struct to
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Glyph {
    img: GlyphImage,
    // reference to the origin on y Axis of the character (top, bottom)
    orig_y: (i16, i16),
    characters: Option<CompactString>,
}

impl Glyph {
    pub fn new(img: GrayImage, orig_y: (i16, i16), characters: Option<CompactString>) -> Self {
        Self {
            img: img.into(),
            orig_y,
            characters,
        }
    }

    pub fn chars(&self) -> Option<&CompactString> {
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

    /// Load Library from a `path` where the library was previously backuped.
    pub fn load_from_path(&mut self, path: impl Into<PathBuf>) -> Result<(), Error> {
        let mut glyph_library_filename = path.into();
        glyph_library_filename.push(LIBRARY_FILENAME);
        let file = File::open(glyph_library_filename).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                Error::NoFileToLoad(source)
            } else {
                Error::FailedToLoadFile(source)
            }
        })?;
        let reader = BufReader::new(file);
        self.load(reader)
    }

    /// Deserialize glyph Library and load it in self
    pub fn load(&mut self, reader: impl Read) -> Result<(), Error> {
        assert!(self.glyphs.is_empty()); //TODO: Report the error to the user
        self.glyphs = ron::de::from_reader(reader).map_err(Error::GlyphRonDeserialization)?;
        Ok(())
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
