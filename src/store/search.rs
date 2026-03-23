use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub enum SearchScope {
    CurrentChannel(Id<ChannelMarker>),
    Server(Id<GuildMarker>),
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub message_id: Id<MessageMarker>,
    pub channel_id: Id<ChannelMarker>,
    pub channel_name: String,
    pub author_name: String,
    pub content_preview: String,
    pub timestamp: String,
}

#[derive(Debug)]
pub struct SearchState {
    pub query: String,
    pub scope: Option<SearchScope>,
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub loading: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            scope: None,
            results: Vec::new(),
            selected: 0,
            loading: false,
        }
    }
}

impl SearchState {
    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
        self.selected = 0;
        self.loading = false;
        self.scope = None;
    }
}
