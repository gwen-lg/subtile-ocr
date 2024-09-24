use crate::ocs::{GlyphCharAsker, Piece};
use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::{prelude::Backend, widgets::Paragraph, Terminal};
use std::{cell::RefCell, ops::DerefMut};

/// TODO
pub struct GlyphAskerTerm<B>
where
    B: Backend,
{
    terminal: RefCell<Terminal<B>>,
}

impl<B> GlyphAskerTerm<B>
where
    B: Backend,
{
    pub fn new(terminal: Terminal<B>) -> Self {
        Self {
            terminal: terminal.into(),
        }
    }
}

impl<B> GlyphCharAsker for GlyphAskerTerm<B>
where
    B: Backend,
{
    /// Note: return String because it can be multiple chars in some case
    /// TODO: use an Array string
    fn ask_char_for_glyph(&self, _piece: &Piece) -> String {
        let mut self_mut = self.terminal.borrow_mut();
        let terminal = self_mut.deref_mut();
        terminal
            .draw(|frame| {
                let ask_msg = Paragraph::new("Press the key corresponding to the glyph!");
                frame.render_widget(ask_msg, frame.area());
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
