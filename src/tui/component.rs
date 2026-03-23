use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;

use crate::discord::actions::Action;
use crate::store::Store;

pub trait Component {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>>;
    fn render(&self, frame: &mut Frame, area: Rect, store: &Store);
}
