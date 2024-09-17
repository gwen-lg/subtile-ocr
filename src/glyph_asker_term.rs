use crate::ocs::{GlyphCharAsker, Piece};
use crossterm::event::{self, KeyCode, KeyEventKind};
use image::DynamicImage;
use ratatui::{prelude::Backend, widgets::Paragraph, Terminal};
use ratatui_image::{picker::Picker, StatefulImage};
use std::{cell::RefCell, ops::DerefMut};

/// TODO
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
    pub fn new(terminal: Terminal<B>, piker: Picker) -> Self {
        Self {
            terminal: (terminal, piker).into(),
        }
    }
}

impl<B> GlyphCharAsker for GlyphAskerTerm<B>
where
    B: Backend,
{
    /// Note: return String because it can be multiple chars in some case
    /// TODO: use an Array string
    fn ask_char_for_glyph(&self, piece: &Piece) -> String {
        let mut self_mut = self.terminal.borrow_mut();
        let (ref mut terminal, ref mut picker) = self_mut.deref_mut();
        terminal
            .draw(|frame| {
                let mut piece_img =
                    picker.new_resize_protocol(DynamicImage::ImageLuma8(piece.img().clone()));
                let msg = Paragraph::new("What is this glyph ?");

                let image = StatefulImage::new(None);
                frame.render_stateful_widget(image, frame.area(), &mut piece_img);
                frame.render_widget(msg, frame.area());
            })
            .unwrap();
        loop {
            if let event::Event::Key(key) = event::read().unwrap() {
                if key.kind == KeyEventKind::Press {
                    if let KeyCode::Char(char) = key.code {
                        let characters = String::from(char);
                        return characters;
                    }
                }
            }
        }
    }
}
