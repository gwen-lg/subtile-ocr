use crate::ocs::{GlyphCharAsker, GlyphResult, Piece};
use compact_str::ToCompactString;
use crossterm::event::{self, KeyCode, KeyEventKind};
use image::{DynamicImage, GrayImage, Pixel, Rgb, RgbImage};
use ratatui::{prelude::Backend, Terminal};
use ratatui_image::{picker::Picker, StatefulImage};
use std::{cell::RefCell, ops::DerefMut};

/// Implementation of `GlyphCharAsker` through a terminal ui.
pub struct GlyphAskerTerm<B>
where
    B: Backend,
{
    terminal: RefCell<(Terminal<B>, Picker)>,
}

impl<B> GlyphAskerTerm<B>
where
    B: Backend,
{
    pub fn new(terminal: Terminal<B>, picker: Picker) -> Self {
        Self {
            terminal: (terminal, picker).into(),
        }
    }
}

impl<B> GlyphCharAsker for GlyphAskerTerm<B>
where
    B: Backend,
{
    /// Note: return a `CompactString` because it can be multiple chars in some case
    fn ask_char_for_glyph(&self, img: &GrayImage, piece: &Piece) -> GlyphResult {
        let mut self_mut = self.terminal.borrow_mut();
        let (ref mut terminal, ref mut picker) = self_mut.deref_mut();
        terminal
            .draw(|frame| {
                let piece_img = piece.img();
                // let mut img = DynamicImage::ImageLuma8(img.clone());
                // img.invert();
                // let mut img = img.into_rgb8();
                let mut sub_img = RgbImage::from_fn(img.width(), img.height(), |x, y| {
                    let mut gray = *img.get_pixel(x, y);
                    gray.invert();
                    gray.to_rgb()
                });

                // set red pixel of piece:
                piece_img
                    .enumerate_pixels()
                    .filter(|(_, _, &pix)| pix.0 == [0])
                    .for_each(|(x, y, _)| {
                        let x = x + u32::from(piece.area().left());
                        let y = y + u32::from(piece.area().top());
                        sub_img.put_pixel(x, y, Rgb([255, 0, 0]))
                    });
                // let inverted_img =
                //     GrayImage::from_fn(piece_img.width(), piece_img.height(), |x, y| {
                //         let mut pixel = *piece_img.get_pixel(x, y);
                //         pixel.invert();
                //         pixel
                //     });
                let mut piece_img = picker.new_resize_protocol(DynamicImage::ImageRgb8(sub_img));
                //let msg = Paragraph::new("What is this glyph ?");

                let image = StatefulImage::new(None);
                frame.render_stateful_widget(image, frame.area(), &mut piece_img);
                //frame.render_widget(msg, frame.area());
            })
            .unwrap();
        loop {
            if let event::Event::Key(key) = event::read().unwrap() {
                if key.kind == KeyEventKind::Press {
                    if let KeyCode::Char(char) = key.code {
                        let characters = char.to_compact_string();
                        return GlyphResult::Char(characters);
                    }
                }
            }
        }
    }
}
