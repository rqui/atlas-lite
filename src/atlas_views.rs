//! Bounded, server-authoritative state for the Atlas Views surface.

use crate::{
    atlas_client::{validate_transport_request, AtlasClientError, TransportRequest},
    atlas_dto::{ViewResultPage, ViewStatus, ViewSummaryPage},
};

/// A compact e-paper list, deliberately smaller than the DTO budget.
pub const VIEW_LIST_LIMIT: usize = 6;
pub const VIEW_RESULT_LIMIT: usize = 12;
pub const VIEW_VISIBLE_ROWS: usize = 6;
/// A single visit may request the first page plus two explicit next pages.
pub const VIEW_RESULT_PAGE_REQUEST_LIMIT: u8 = 3;
pub const VIEW_TITLE_MAX_BYTES: usize = 72;
pub const VIEW_PATH_MAX_BYTES: usize = 96;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AtlasViewsFocus {
    #[default]
    List,
    Results,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AtlasViewsRequest {
    List,
    Results { id: String, cursor: Option<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasViewSummary {
    id: String,
    name: String,
    valid: bool,
}

impl AtlasViewSummary {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub const fn valid(&self) -> bool {
        self.valid
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasViewResult {
    id: String,
    title: String,
    path: String,
}

impl AtlasViewResult {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// State owned only by Views. It never renders View values or desktop layouts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AtlasViewsState {
    views: Vec<AtlasViewSummary>,
    selected_view: usize,
    results: Vec<AtlasViewResult>,
    selected_result: usize,
    window_offset: usize,
    selected_view_id: Option<String>,
    selected_view_name: String,
    next_cursor: Option<String>,
    page_number: u8,
    page_requests: u8,
    focus: AtlasViewsFocus,
    pending_view_id: Option<String>,
    pending_view_name: Option<String>,
}

impl AtlasViewsState {
    #[must_use]
    pub fn views(&self) -> &[AtlasViewSummary] {
        &self.views
    }
    #[must_use]
    pub fn results(&self) -> &[AtlasViewResult] {
        &self.results
    }
    #[must_use]
    pub const fn selected_view(&self) -> usize {
        self.selected_view
    }
    #[must_use]
    pub const fn selected_result(&self) -> usize {
        self.selected_result
    }
    #[must_use]
    pub const fn window_offset(&self) -> usize {
        self.window_offset
    }
    #[must_use]
    pub const fn focus(&self) -> AtlasViewsFocus {
        self.focus
    }
    #[must_use]
    pub fn selected_view_name(&self) -> &str {
        &self.selected_view_name
    }
    #[must_use]
    pub const fn page_number(&self) -> u8 {
        self.page_number
    }
    #[must_use]
    pub const fn page_requests(&self) -> u8 {
        self.page_requests
    }
    #[must_use]
    pub fn has_next_page(&self) -> bool {
        self.next_cursor.is_some()
    }
    #[must_use]
    pub fn pagination_incomplete(&self) -> bool {
        self.has_next_page()
    }
    #[must_use]
    pub const fn pagination_cap_reached(&self) -> bool {
        self.page_requests >= VIEW_RESULT_PAGE_REQUEST_LIMIT
    }
    #[must_use]
    pub fn next_page_available(&self) -> bool {
        self.has_next_page() && !self.pagination_cap_reached()
    }

    pub fn replace_views(&mut self, response: ViewSummaryPage) {
        self.views = response
            .items
            .into_iter()
            .filter_map(|view| {
                let valid_id = validate_transport_request(&TransportRequest::GetViewResults {
                    id: view.id.clone(),
                    cursor: None,
                    limit: VIEW_RESULT_LIMIT,
                })
                .is_ok();
                Some(AtlasViewSummary {
                    id: view.id,
                    name: bounded_text(&view.name, VIEW_TITLE_MAX_BYTES),
                    valid: valid_id && view.status == ViewStatus::Ok,
                })
            })
            .take(VIEW_LIST_LIMIT)
            .collect();
        self.selected_view = 0;
        self.focus = AtlasViewsFocus::List;
    }

    /// Starts an explicit first-page request. Switching views cannot show a
    /// previous View's results; retrying the current View retains its safe page.
    pub fn select_view_request(&mut self) -> Option<AtlasViewsRequest> {
        let view = self.views.get(self.selected_view)?;
        if !view.valid {
            return None;
        }
        // A fresh cursor=None is staged as a new bounded session, including
        // when the user reopens the same View after reaching the page cap.
        // The current snapshot remains coherent if this request fails.
        self.pending_view_id = Some(view.id.clone());
        self.pending_view_name = Some(view.name.clone());
        self.focus = AtlasViewsFocus::Results;
        Some(AtlasViewsRequest::Results {
            id: view.id.clone(),
            cursor: None,
        })
    }

    pub fn next_page_request(&self) -> Option<AtlasViewsRequest> {
        if !self.next_page_available() {
            return None;
        }
        Some(AtlasViewsRequest::Results {
            id: self.selected_view_id.clone()?,
            cursor: Some(self.next_cursor.clone()?),
        })
    }

    pub fn replace_results(
        &mut self,
        response: ViewResultPage,
        requested_view_id: &str,
        first_page: bool,
    ) -> Result<(), AtlasClientError> {
        let expected_view_id = if first_page {
            self.pending_view_id.as_deref()
        } else {
            self.selected_view_id.as_deref()
        }
        .ok_or(AtlasClientError::MalformedPayload)?;
        if expected_view_id != requested_view_id
            || response.view.id != requested_view_id
            || response.view.status != ViewStatus::Ok
        {
            return Err(AtlasClientError::MalformedPayload);
        }
        // Validate server pagination metadata before changing any retained
        // page, cursor, selection, or request-budget field.
        validate_transport_request(&TransportRequest::GetViewResults {
            id: requested_view_id.to_owned(),
            cursor: response.next_cursor.clone(),
            limit: VIEW_RESULT_LIMIT,
        })
        .map_err(|_| AtlasClientError::MalformedPayload)?;
        let results = response
            .items
            .into_iter()
            .filter_map(|result| {
                let id = result.id?;
                validate_transport_request(&TransportRequest::GetNote { id: id.clone() }).ok()?;
                Some(AtlasViewResult {
                    id,
                    title: bounded_text(&result.title, VIEW_TITLE_MAX_BYTES),
                    path: bounded_text(&result.path, VIEW_PATH_MAX_BYTES),
                })
            })
            .take(VIEW_RESULT_LIMIT)
            .collect();
        if first_page {
            self.selected_view_id = Some(requested_view_id.to_owned());
            self.selected_view_name = self
                .pending_view_name
                .take()
                .unwrap_or_else(|| bounded_text(&response.view.name, VIEW_TITLE_MAX_BYTES));
            self.pending_view_id = None;
            self.page_requests = 0;
            self.page_number = 0;
            self.next_cursor = None;
        }
        self.results = results;
        self.next_cursor = response.next_cursor;
        self.page_requests = self.page_requests.saturating_add(1);
        self.page_number = self.page_requests;
        self.selected_result = 0;
        self.window_offset = 0;
        self.focus = AtlasViewsFocus::Results;
        Ok(())
    }

    pub fn abort_pending_view_session(&mut self) {
        self.pending_view_id = None;
        self.pending_view_name = None;
    }

    pub fn move_previous(&mut self) {
        let count = self.selection_count();
        if count == 0 {
            return;
        }
        self.selected_result = self.selected_result.checked_sub(1).unwrap_or(count - 1);
        self.update_window();
    }
    pub fn move_next(&mut self) {
        let count = self.selection_count();
        if count == 0 {
            return;
        }
        self.selected_result = (self.selected_result + 1) % count;
        self.update_window();
    }
    pub fn move_view_previous(&mut self) {
        if self.views.is_empty() {
            return;
        }
        self.selected_view = self
            .selected_view
            .checked_sub(1)
            .unwrap_or(self.views.len() - 1);
    }
    pub fn move_view_next(&mut self) {
        if self.views.is_empty() {
            return;
        }
        self.selected_view = (self.selected_view + 1) % self.views.len();
    }
    #[must_use]
    pub fn selected_note_id(&self) -> Option<&str> {
        self.results
            .get(self.selected_result)
            .map(AtlasViewResult::id)
    }
    #[must_use]
    pub fn next_page_selected(&self) -> bool {
        self.selected_result >= self.results.len()
    }

    fn selection_count(&self) -> usize {
        self.results.len() + usize::from(self.next_page_available())
    }
    fn update_window(&mut self) {
        let count = self.selection_count();
        let max = count.saturating_sub(VIEW_VISIBLE_ROWS);
        if self.selected_result < self.window_offset {
            self.window_offset = self.selected_result;
        } else if self.selected_result + 1 > self.window_offset + VIEW_VISIBLE_ROWS {
            self.window_offset = self.selected_result + 1 - VIEW_VISIBLE_ROWS;
        }
        self.window_offset = self.window_offset.min(max);
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
