use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::store::state::{FocusTarget, InputMode};

const CHORD_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    // Navigation
    FocusSidebar,
    ToggleMemberSidebar,
    OpenCommandPalette,
    MoveUp,
    MoveDown,
    Select,
    Back,
    CycleFocusForward,
    CycleFocusBackward,
    // Messages
    EnterInsertMode,
    Reply,
    EditMessage,
    DeleteMessage,
    AddReaction,
    JumpToTop,
    JumpToBottom,
    PageUp,
    PageDown,
    YankMessage,
    OpenSearch,
    NextSearchResult,
    PrevSearchResult,
    // Insert
    SendMessage,
    InsertNewline,
    ExitInsertMode,
    // Quit
    Quit,
    // Passthrough
    Unhandled(KeyEvent),
}

pub struct KeyDispatcher {
    pending_chord: Option<(KeyCode, Instant)>,
}

impl KeyDispatcher {
    pub fn new() -> Self {
        Self {
            pending_chord: None,
        }
    }

    pub fn dispatch(
        &mut self,
        key: KeyEvent,
        mode: InputMode,
        _focus: FocusTarget,
    ) -> KeyAction {
        // Ctrl bindings work in all modes
        if key.modifiers == KeyModifiers::CONTROL {
            match key.code {
                KeyCode::Char('s') => return KeyAction::FocusSidebar,
                KeyCode::Char('m') => return KeyAction::ToggleMemberSidebar,
                KeyCode::Char('k') => return KeyAction::OpenCommandPalette,
                KeyCode::Char('c') => return KeyAction::Quit,
                KeyCode::Char('u') => return KeyAction::PageUp,
                KeyCode::Char('d') => return KeyAction::PageDown,
                _ => {}
            }
        }

        match mode {
            InputMode::Insert => self.dispatch_insert(key),
            InputMode::Normal => self.dispatch_normal(key),
        }
    }

    fn dispatch_insert(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Enter if key.modifiers == KeyModifiers::SHIFT => KeyAction::InsertNewline,
            KeyCode::Enter => KeyAction::SendMessage,
            KeyCode::Esc => KeyAction::ExitInsertMode,
            _ => KeyAction::Unhandled(key),
        }
    }

    fn dispatch_normal(&mut self, key: KeyEvent) -> KeyAction {
        // Handle pending chord
        if let Some((chord_key, chord_time)) = self.pending_chord.take() {
            if chord_time.elapsed() < CHORD_TIMEOUT {
                // We have a pending chord - check if this key completes it
                if chord_key == KeyCode::Char('g') && key.code == KeyCode::Char('g') {
                    return KeyAction::JumpToTop;
                }
                // Chord didn't match or timed out - fall through and process
                // the current key normally (chord is already cleared via take())
            }
            // Expired or unmatched chord - process the current key normally below
        }

        // No modifiers for normal mode single-key bindings
        if key.modifiers == KeyModifiers::NONE {
            match key.code {
                KeyCode::Char('j') => return KeyAction::MoveDown,
                KeyCode::Char('k') => return KeyAction::MoveUp,
                KeyCode::Enter => return KeyAction::Select,
                KeyCode::Esc => return KeyAction::Back,
                KeyCode::Tab => return KeyAction::CycleFocusForward,
                KeyCode::Char('i') => return KeyAction::EnterInsertMode,
                KeyCode::Char('r') => return KeyAction::Reply,
                KeyCode::Char('e') => return KeyAction::EditMessage,
                KeyCode::Char('d') => return KeyAction::DeleteMessage,
                KeyCode::Char('+') => return KeyAction::AddReaction,
                KeyCode::Char('G') => return KeyAction::JumpToBottom,
                KeyCode::Char('y') => return KeyAction::YankMessage,
                KeyCode::Char('/') => return KeyAction::OpenSearch,
                KeyCode::Char('n') => return KeyAction::NextSearchResult,
                KeyCode::Char('q') => return KeyAction::Quit,
                // Start chord
                KeyCode::Char('g') => {
                    self.pending_chord = Some((KeyCode::Char('g'), Instant::now()));
                    // Return Unhandled while waiting for chord completion
                    return KeyAction::Unhandled(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
                }
                _ => {}
            }
        }

        // Shift+Tab for CycleFocusBackward
        if key.modifiers == KeyModifiers::SHIFT && key.code == KeyCode::BackTab {
            return KeyAction::CycleFocusBackward;
        }

        // Shift+N for PrevSearchResult
        if key.modifiers == KeyModifiers::NONE && key.code == KeyCode::Char('N') {
            return KeyAction::PrevSearchResult;
        }

        KeyAction::Unhandled(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    #[test]
    fn ctrl_bindings_work_in_normal_mode() {
        let mut d = KeyDispatcher::new();
        assert_eq!(d.dispatch(ctrl('s'), InputMode::Normal, FocusTarget::MessageList), KeyAction::FocusSidebar);
        assert_eq!(d.dispatch(ctrl('m'), InputMode::Normal, FocusTarget::MessageList), KeyAction::ToggleMemberSidebar);
        assert_eq!(d.dispatch(ctrl('k'), InputMode::Normal, FocusTarget::MessageList), KeyAction::OpenCommandPalette);
        assert_eq!(d.dispatch(ctrl('c'), InputMode::Normal, FocusTarget::MessageList), KeyAction::Quit);
        assert_eq!(d.dispatch(ctrl('u'), InputMode::Normal, FocusTarget::MessageList), KeyAction::PageUp);
        assert_eq!(d.dispatch(ctrl('d'), InputMode::Normal, FocusTarget::MessageList), KeyAction::PageDown);
    }

    #[test]
    fn ctrl_bindings_work_in_insert_mode() {
        let mut d = KeyDispatcher::new();
        assert_eq!(d.dispatch(ctrl('s'), InputMode::Insert, FocusTarget::MessageInput), KeyAction::FocusSidebar);
        assert_eq!(d.dispatch(ctrl('m'), InputMode::Insert, FocusTarget::MessageInput), KeyAction::ToggleMemberSidebar);
        assert_eq!(d.dispatch(ctrl('k'), InputMode::Insert, FocusTarget::MessageInput), KeyAction::OpenCommandPalette);
        assert_eq!(d.dispatch(ctrl('c'), InputMode::Insert, FocusTarget::MessageInput), KeyAction::Quit);
        assert_eq!(d.dispatch(ctrl('u'), InputMode::Insert, FocusTarget::MessageInput), KeyAction::PageUp);
        assert_eq!(d.dispatch(ctrl('d'), InputMode::Insert, FocusTarget::MessageInput), KeyAction::PageDown);
    }

    #[test]
    fn normal_mode_single_keys() {
        let mut d = KeyDispatcher::new();
        assert_eq!(d.dispatch(key(KeyCode::Char('j')), InputMode::Normal, FocusTarget::MessageList), KeyAction::MoveDown);
        assert_eq!(d.dispatch(key(KeyCode::Char('k')), InputMode::Normal, FocusTarget::MessageList), KeyAction::MoveUp);
        assert_eq!(d.dispatch(key(KeyCode::Enter), InputMode::Normal, FocusTarget::MessageList), KeyAction::Select);
        assert_eq!(d.dispatch(key(KeyCode::Esc), InputMode::Normal, FocusTarget::MessageList), KeyAction::Back);
        assert_eq!(d.dispatch(key(KeyCode::Tab), InputMode::Normal, FocusTarget::MessageList), KeyAction::CycleFocusForward);
        assert_eq!(d.dispatch(key(KeyCode::Char('i')), InputMode::Normal, FocusTarget::MessageList), KeyAction::EnterInsertMode);
        assert_eq!(d.dispatch(key(KeyCode::Char('r')), InputMode::Normal, FocusTarget::MessageList), KeyAction::Reply);
        assert_eq!(d.dispatch(key(KeyCode::Char('e')), InputMode::Normal, FocusTarget::MessageList), KeyAction::EditMessage);
        assert_eq!(d.dispatch(key(KeyCode::Char('d')), InputMode::Normal, FocusTarget::MessageList), KeyAction::DeleteMessage);
        assert_eq!(d.dispatch(key(KeyCode::Char('+')), InputMode::Normal, FocusTarget::MessageList), KeyAction::AddReaction);
        assert_eq!(d.dispatch(key(KeyCode::Char('G')), InputMode::Normal, FocusTarget::MessageList), KeyAction::JumpToBottom);
        assert_eq!(d.dispatch(key(KeyCode::Char('y')), InputMode::Normal, FocusTarget::MessageList), KeyAction::YankMessage);
        assert_eq!(d.dispatch(key(KeyCode::Char('/')), InputMode::Normal, FocusTarget::MessageList), KeyAction::OpenSearch);
        assert_eq!(d.dispatch(key(KeyCode::Char('n')), InputMode::Normal, FocusTarget::MessageList), KeyAction::NextSearchResult);
        assert_eq!(d.dispatch(key(KeyCode::Char('N')), InputMode::Normal, FocusTarget::MessageList), KeyAction::PrevSearchResult);
    }

    #[test]
    fn normal_mode_shift_tab() {
        let mut d = KeyDispatcher::new();
        assert_eq!(
            d.dispatch(shift(KeyCode::BackTab), InputMode::Normal, FocusTarget::MessageList),
            KeyAction::CycleFocusBackward
        );
    }

    #[test]
    fn insert_mode_passthrough() {
        let mut d = KeyDispatcher::new();
        let a_key = key(KeyCode::Char('a'));
        assert_eq!(d.dispatch(a_key, InputMode::Insert, FocusTarget::MessageInput), KeyAction::Unhandled(a_key));
        let z_key = key(KeyCode::Char('z'));
        assert_eq!(d.dispatch(z_key, InputMode::Insert, FocusTarget::MessageInput), KeyAction::Unhandled(z_key));
        let left = key(KeyCode::Left);
        assert_eq!(d.dispatch(left, InputMode::Insert, FocusTarget::MessageInput), KeyAction::Unhandled(left));
    }

    #[test]
    fn insert_mode_enter_sends_message() {
        let mut d = KeyDispatcher::new();
        assert_eq!(d.dispatch(key(KeyCode::Enter), InputMode::Insert, FocusTarget::MessageInput), KeyAction::SendMessage);
    }

    #[test]
    fn insert_mode_shift_enter_inserts_newline() {
        let mut d = KeyDispatcher::new();
        assert_eq!(d.dispatch(shift(KeyCode::Enter), InputMode::Insert, FocusTarget::MessageInput), KeyAction::InsertNewline);
    }

    #[test]
    fn insert_mode_esc_exits() {
        let mut d = KeyDispatcher::new();
        assert_eq!(d.dispatch(key(KeyCode::Esc), InputMode::Insert, FocusTarget::MessageInput), KeyAction::ExitInsertMode);
    }

    #[test]
    fn chord_gg_jumps_to_top() {
        let mut d = KeyDispatcher::new();
        // First g starts the chord
        let first_g = d.dispatch(key(KeyCode::Char('g')), InputMode::Normal, FocusTarget::MessageList);
        // First g returns Unhandled while waiting for chord
        assert!(matches!(first_g, KeyAction::Unhandled(_)));
        assert!(d.pending_chord.is_some());
        // Second g within timeout completes the chord
        assert_eq!(
            d.dispatch(key(KeyCode::Char('g')), InputMode::Normal, FocusTarget::MessageList),
            KeyAction::JumpToTop
        );
        assert!(d.pending_chord.is_none());
    }

    #[test]
    fn chord_g_then_other_cancels() {
        let mut d = KeyDispatcher::new();
        // First g starts the chord
        d.dispatch(key(KeyCode::Char('g')), InputMode::Normal, FocusTarget::MessageList);
        // j cancels the chord and is processed as MoveDown
        assert_eq!(
            d.dispatch(key(KeyCode::Char('j')), InputMode::Normal, FocusTarget::MessageList),
            KeyAction::MoveDown
        );
        assert!(d.pending_chord.is_none());
    }

    #[test]
    fn chord_timeout() {
        let mut d = KeyDispatcher::new();
        // Manually insert an expired chord
        d.pending_chord = Some((KeyCode::Char('g'), Instant::now() - Duration::from_millis(600)));
        // Next key should be processed normally, not as chord completion
        assert_eq!(
            d.dispatch(key(KeyCode::Char('g')), InputMode::Normal, FocusTarget::MessageList),
            // After expired chord is discarded, 'g' starts a new chord (returns Unhandled)
            KeyAction::Unhandled(key(KeyCode::Char('g')))
        );
    }

    #[test]
    fn quit_ctrl_c_normal_mode() {
        let mut d = KeyDispatcher::new();
        assert_eq!(d.dispatch(ctrl('c'), InputMode::Normal, FocusTarget::MessageList), KeyAction::Quit);
    }

    #[test]
    fn quit_ctrl_c_insert_mode() {
        let mut d = KeyDispatcher::new();
        assert_eq!(d.dispatch(ctrl('c'), InputMode::Insert, FocusTarget::MessageInput), KeyAction::Quit);
    }

    #[test]
    fn quit_q_normal_mode() {
        let mut d = KeyDispatcher::new();
        assert_eq!(d.dispatch(key(KeyCode::Char('q')), InputMode::Normal, FocusTarget::MessageList), KeyAction::Quit);
    }
}
