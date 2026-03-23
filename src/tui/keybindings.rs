/// Actions that components can return to the App for cross-component handling.
/// Most key handling now lives in individual components; these actions are for
/// cases where a component needs the App to coordinate across multiple components
/// (e.g., reply sets up both message list and message input state).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Reply,
    EditMessage,
    DeleteMessage,
    OpenEmojiPicker,
    /// Open the profile overlay for the given user.
    OpenProfileOverlay {
        user_id: twilight_model::id::Id<twilight_model::id::marker::UserMarker>,
    },
}
