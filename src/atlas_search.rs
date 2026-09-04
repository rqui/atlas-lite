//! Bounded, server-authoritative state for the Atlas Search surface.

use crate::{
    atlas_client::{validate_transport_request, TransportRequest, MAX_SEARCH_QUERY_BYTES},
    atlas_dto::SearchResponse,
    keyboard_navigation::KeyboardGridNavigation,
};

/// The e-paper Search surface retains a short, scrollable page rather than a
/// server-sized result set.
pub const SEARCH_RESULT_LIMIT: usize = 12;
pub const SEARCH_VISIBLE_ROWS: usize = 6;
pub const SEARCH_TITLE_MAX_BYTES: usize = 72;
pub const SEARCH_SNIPPET_MAX_BYTES: usize = 120;
pub const SEARCH_KEY_ROWS: [[&str; 6]; 5] = [
    ["A", "B", "C", "D", "E", "F"],
    ["G", "H", "I", "J", "K", "L"],
    ["M", "N", "O", "P", "Q", "R"],
    ["S", "T", "U", "V", "W", "X"],
    ["Y", "Z", "SPC", "DEL", "CLR", "GO"],
];
const SEARCH_KEY_COLUMNS: usize = 6;
const SEARCH_KEY_COUNT: usize = 30;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AtlasSearchFocus {
    #[default]
    Input,
    Results,
}

/// A display-safe search hit. Only a stable Atlas ID can be opened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasSearchResult {
    id: String,
    title: String,
    snippet: String,
}

impl AtlasSearchResult {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn snippet(&self) -> &str {
        &self.snippet
    }
}

/// State owned exclusively by Search. It is intentionally separate from the
/// Home and Library snapshots so an error here cannot relabel those surfaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasSearchState {
    query: String,
    /// Query that produced the retained safe hits. Results remain usable only
    /// while it matches the editable query shown to the user.
    results_query: Option<String>,
    keyboard_navigation: KeyboardGridNavigation,
    results: Vec<AtlasSearchResult>,
    selected: usize,
    window_offset: usize,
    focus: AtlasSearchFocus,
    index_not_ready: bool,
    retry_after_seconds: Option<u32>,
}

impl Default for AtlasSearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            results_query: None,
            keyboard_navigation: KeyboardGridNavigation::new(SEARCH_KEY_COUNT, SEARCH_KEY_COLUMNS),
            results: Vec::new(),
            selected: 0,
            window_offset: 0,
            focus: AtlasSearchFocus::Input,
            index_not_ready: false,
            retry_after_seconds: None,
        }
    }
}

impl AtlasSearchState {
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    #[must_use]
    pub const fn keyboard_navigation(&self) -> KeyboardGridNavigation {
        self.keyboard_navigation
    }

    #[must_use]
    pub fn results(&self) -> &[AtlasSearchResult] {
        &self.results
    }

    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    #[must_use]
    pub const fn window_offset(&self) -> usize {
        self.window_offset
    }

    #[must_use]
    pub const fn focus(&self) -> AtlasSearchFocus {
        self.focus
    }

    #[must_use]
    pub const fn retry_after_seconds(&self) -> Option<u32> {
        self.retry_after_seconds
    }

    #[must_use]
    pub const fn index_not_ready(&self) -> bool {
        self.index_not_ready
    }

    #[must_use]
    pub const fn selected_key_label(&self) -> &'static str {
        SEARCH_KEY_ROWS[self.keyboard_navigation.selected() / SEARCH_KEY_COLUMNS]
            [self.keyboard_navigation.selected() % SEARCH_KEY_COLUMNS]
    }

    pub fn toggle_keyboard_axis(&mut self) {
        self.keyboard_navigation.toggle_axis();
    }

    pub fn move_key_previous(&mut self) {
        self.keyboard_navigation.move_previous();
    }

    pub fn move_key_next(&mut self) {
        self.keyboard_navigation.move_next();
    }

    /// Applies one selected keyboard key. `true` requests one explicit search;
    /// an empty query is rejected locally and never reaches the transport.
    pub fn apply_selected_key(&mut self) -> bool {
        match self.selected_key_label() {
            "DEL" => {
                self.query.pop();
                self.invalidate_results_for_changed_query();
                self.clear_index_not_ready();
                false
            }
            "CLR" => {
                self.query.clear();
                self.results.clear();
                self.selected = 0;
                self.window_offset = 0;
                self.focus = AtlasSearchFocus::Input;
                self.clear_index_not_ready();
                false
            }
            "GO" => !self.query.is_empty(),
            "SPC" => {
                self.push_query(" ");
                false
            }
            character => {
                self.push_query(character);
                false
            }
        }
    }

    /// Host/simulator helper and Unicode boundary: truncates only at UTF-8
    /// character boundaries, never producing an invalid query string.
    pub fn set_query(&mut self, query: &str) {
        self.query = bounded_text(query, MAX_SEARCH_QUERY_BYTES);
        self.invalidate_results_for_changed_query();
        self.clear_index_not_ready();
    }

    pub fn set_index_not_ready(&mut self, retry_after_seconds: Option<u32>) {
        self.index_not_ready = true;
        self.retry_after_seconds = retry_after_seconds;
    }

    pub fn clear_index_not_ready(&mut self) {
        self.index_not_ready = false;
        self.retry_after_seconds = None;
    }

    pub fn replace_response(&mut self, response: SearchResponse) {
        self.results = response
            .hits
            .into_iter()
            .filter_map(|hit| {
                let id = hit.id?;
                validate_transport_request(&TransportRequest::GetNote { id: id.clone() }).ok()?;
                Some(AtlasSearchResult {
                    id,
                    title: bounded_text(&hit.title, SEARCH_TITLE_MAX_BYTES),
                    snippet: bounded_text(&hit.snippet, SEARCH_SNIPPET_MAX_BYTES),
                })
            })
            .take(SEARCH_RESULT_LIMIT)
            .collect();
        self.results_query = Some(self.query.clone());
        self.selected = 0;
        self.window_offset = 0;
        self.focus = AtlasSearchFocus::Results;
        self.clear_index_not_ready();
    }

    pub fn move_result_previous(&mut self) {
        let selection_count = self.results.len() + 1;
        self.selected = self.selected.checked_sub(1).unwrap_or(selection_count - 1);
        self.update_window();
    }

    pub fn move_result_next(&mut self) {
        let selection_count = self.results.len() + 1;
        self.selected = (self.selected + 1) % selection_count;
        self.update_window();
    }

    #[must_use]
    pub const fn refine_selected(&self) -> bool {
        self.selected >= self.results.len()
    }

    pub fn focus_input(&mut self) {
        self.focus = AtlasSearchFocus::Input;
    }

    pub fn focus_results(&mut self) {
        self.focus = AtlasSearchFocus::Results;
    }

    #[must_use]
    pub fn selected_id(&self) -> Option<&str> {
        self.results.get(self.selected).map(AtlasSearchResult::id)
    }

    fn push_query(&mut self, value: &str) {
        if self.query.len().saturating_add(value.len()) <= MAX_SEARCH_QUERY_BYTES {
            self.query.push_str(value);
            self.invalidate_results_for_changed_query();
            self.clear_index_not_ready();
        }
    }

    fn invalidate_results_for_changed_query(&mut self) {
        if self.results_query.as_deref() == Some(self.query()) {
            return;
        }
        self.results.clear();
        self.results_query = None;
        self.selected = 0;
        self.window_offset = 0;
    }

    fn update_window(&mut self) {
        let max_offset = (self.results.len() + 1).saturating_sub(SEARCH_VISIBLE_ROWS);
        if self.selected < self.window_offset {
            self.window_offset = self.selected;
        } else if self.selected + 1 > self.window_offset + SEARCH_VISIBLE_ROWS {
            self.window_offset = self.selected + 1 - SEARCH_VISIBLE_ROWS;
        }
        self.window_offset = self.window_offset.min(max_offset);
    }
}

fn bounded_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.into();
    }
    let mut end = max_bytes.saturating_sub('…'.len_utf8());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::{AtlasSearchState, SEARCH_RESULT_LIMIT, SEARCH_TITLE_MAX_BYTES};
    use crate::atlas_dto::{SearchHit, SearchResponse};

    #[test]
    fn unicode_query_is_bounded_on_character_boundaries() {
        let mut search = AtlasSearchState::default();
        search.set_query(&"é".repeat(100));
        assert!(search.query().len() <= 128);
        assert!(search.query().is_char_boundary(search.query().len()));
    }

    #[test]
    fn retains_only_openable_bounded_hits() {
        let mut search = AtlasSearchState::default();
        search.replace_response(SearchResponse {
            query: "x".into(),
            total: 99,
            hits: (0..SEARCH_RESULT_LIMIT + 2)
                .map(|index| SearchHit {
                    id: Some(format!("00000000-0000-4000-8000-{index:012}")),
                    path: "ignored".into(),
                    title: "x".repeat(SEARCH_TITLE_MAX_BYTES + 10),
                    snippet: "snippet".into(),
                    revision: "r1".into(),
                    state: None,
                })
                .collect(),
        });
        assert_eq!(search.results().len(), SEARCH_RESULT_LIMIT);
        assert!(search.results()[0].title().len() <= SEARCH_TITLE_MAX_BYTES);
    }

    #[test]
    fn bounded_refine_action_is_reachable_after_results() {
        let mut search = AtlasSearchState::default();
        search.replace_response(SearchResponse {
            query: "x".into(),
            total: 1,
            hits: vec![SearchHit {
                id: Some("00000000-0000-4000-8000-000000000001".into()),
                path: "ignored".into(),
                title: "One".into(),
                snippet: "Snippet".into(),
                revision: "r1".into(),
                state: None,
            }],
        });
        search.move_result_next();
        assert!(search.refine_selected());
        assert_eq!(search.selected_id(), None);
        search.focus_input();
        assert_eq!(search.focus(), super::AtlasSearchFocus::Input);
    }
}
