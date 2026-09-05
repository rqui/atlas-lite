//! Bounded, defensive Atlas Library hierarchy derived from note summaries.

use std::collections::{BTreeMap, BTreeSet};

use crate::atlas_dto::NoteSummaryPage;

/// Absolute number of summaries retained for the Library on this device.
pub const LIBRARY_NODE_LIMIT: usize = 64;
/// Maximum UTF-8 bytes accepted for an opaque Atlas ID or parent reference.
pub const LIBRARY_ID_MAX_BYTES: usize = 128;
/// Maximum UTF-8 bytes retained for one rendered Library title.
pub const LIBRARY_TITLE_MAX_BYTES: usize = 96;
/// Path is retained only as the stable web-order tie-breaker, never rendered.
/// Atlas Lite refuses rather than truncates a longer sort key: truncation
/// would silently produce a different order than Atlas Web.
pub const LIBRARY_PATH_MAX_BYTES: usize = 1024;
/// Maximum bytes retained for one Atlas sibling-order key.
pub const LIBRARY_ORDER_MAX_BYTES: usize = 64;
/// The bounded number of summaries requested from one Atlas page.
pub const LIBRARY_PAGE_SIZE: usize = 16;
/// The bounded number of Atlas pages an explicit Library refresh may request.
pub const LIBRARY_PAGE_LIMIT: usize = 4;
/// Number of hierarchy rows available in the e-paper Library viewport.
pub const LIBRARY_VISIBLE_ROWS: usize = 12;

/// A safe statement about whether the locally rendered hierarchy is whole.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryCompleteness {
    Complete,
    CursorRemaining,
    NodeBudgetReached,
}

/// Non-fatal structural conditions withheld from the rendered tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryIssue {
    MissingId,
    InvalidId,
    DuplicateId,
    MissingParent,
    InvalidParent,
    UnsupportedPathOrder,
    OrderTooLong,
    Cycle,
    NodeBudgetReached,
}

/// The only summary fields retained by the Library.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryNode {
    id: String,
    parent_id: Option<String>,
    order: Option<String>,
    path: String,
    title: String,
}

impl LibraryNode {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn parent_id(&self) -> Option<&str> {
        self.parent_id.as_deref()
    }

    #[must_use]
    pub fn order(&self) -> Option<&str> {
        self.order.as_deref()
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
}

/// A bounded, non-recursive hierarchy for the e-paper Library surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryHierarchy {
    nodes: Vec<LibraryNode>,
    root_ids: Vec<String>,
    children: BTreeMap<String, Vec<String>>,
    completeness: LibraryCompleteness,
    issues: Vec<LibraryIssue>,
}

impl Default for LibraryHierarchy {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            root_ids: Vec::new(),
            children: BTreeMap::new(),
            completeness: LibraryCompleteness::Complete,
            issues: Vec::new(),
        }
    }
}

impl LibraryHierarchy {
    /// Builds one bounded hierarchy from already-fetched client pages.
    #[must_use]
    pub fn from_pages(pages: &[NoteSummaryPage]) -> Self {
        let mut hierarchy = Self::default();
        let mut ids = BTreeSet::new();

        for page in pages {
            for summary in &page.items {
                if hierarchy.nodes.len() == LIBRARY_NODE_LIMIT {
                    hierarchy.completeness = LibraryCompleteness::NodeBudgetReached;
                    hierarchy.record_issue(LibraryIssue::NodeBudgetReached);
                    break;
                }

                let Some(id) = summary.id.as_deref() else {
                    hierarchy.record_issue(LibraryIssue::MissingId);
                    continue;
                };
                if !is_atlas_id(id) {
                    hierarchy.record_issue(LibraryIssue::InvalidId);
                    continue;
                }
                if ids.contains(id) {
                    hierarchy.record_issue(LibraryIssue::DuplicateId);
                    continue;
                }

                let parent_id = match summary.parent_id.as_deref() {
                    None => None,
                    Some(parent_id) if is_atlas_id(parent_id) => Some(parent_id.to_owned()),
                    Some(_) => {
                        hierarchy.record_issue(LibraryIssue::InvalidParent);
                        continue;
                    }
                };
                ids.insert(id.to_owned());
                let order = match summary.order.as_deref() {
                    Some(order) if !is_canonical_order(order) => {
                        hierarchy.record_issue(LibraryIssue::OrderTooLong);
                        None
                    }
                    Some(order) => Some(order.to_owned()),
                    None => None,
                };

                if !is_supported_order_path(&summary.path) {
                    hierarchy.record_issue(LibraryIssue::UnsupportedPathOrder);
                    continue;
                }

                hierarchy.nodes.push(LibraryNode {
                    id: id.to_owned(),
                    parent_id,
                    order,
                    path: summary.path.clone(),
                    title: bounded_title(&summary.title),
                });
            }
            if hierarchy.completeness == LibraryCompleteness::NodeBudgetReached {
                break;
            }
        }

        if hierarchy.completeness == LibraryCompleteness::Complete
            && pages.last().is_some_and(|page| page.next_cursor.is_some())
        {
            hierarchy.completeness = LibraryCompleteness::CursorRemaining;
        }

        hierarchy.link_safe_nodes();
        hierarchy
    }

    #[must_use]
    pub fn nodes(&self) -> &[LibraryNode] {
        &self.nodes
    }

    #[must_use]
    pub fn root_ids(&self) -> &[String] {
        &self.root_ids
    }

    #[must_use]
    pub fn child_ids(&self, id: &str) -> &[String] {
        self.children.get(id).map_or(&[], Vec::as_slice)
    }

    /// Flatten the bounded visible hierarchy in render order. The returned
    /// values are stable Atlas IDs; paths are never structural identity.
    #[must_use]
    pub fn visible_ids(&self) -> Vec<&str> {
        let mut ids = Vec::with_capacity(self.nodes.len());
        let mut pending: Vec<&str> = self.root_ids.iter().rev().map(String::as_str).collect();

        while let Some(id) = pending.pop() {
            ids.push(id);
            for child_id in self.child_ids(id).iter().rev() {
                pending.push(child_id);
            }
        }
        ids
    }

    #[must_use]
    pub const fn completeness(&self) -> LibraryCompleteness {
        self.completeness
    }

    #[must_use]
    pub fn issues(&self) -> &[LibraryIssue] {
        &self.issues
    }

    fn link_safe_nodes(&mut self) {
        let known_ids: BTreeSet<_> = self.nodes.iter().map(|node| node.id.clone()).collect();
        let mut blocked = BTreeSet::new();
        let mut structural_issues = Vec::new();

        for node in &self.nodes {
            if let Some(parent_id) = node.parent_id.as_deref() {
                if !known_ids.contains(parent_id) {
                    // A bounded, cursor-truncated fetch can legitimately omit
                    // the parent. Keep the child visible as a provisional root
                    // instead of silently hiding a valid note.
                    if self.completeness == LibraryCompleteness::Complete {
                        blocked.insert(node.id.clone());
                    }
                    structural_issues.push(LibraryIssue::MissingParent);
                }
            }
        }

        for node in &self.nodes {
            if has_cycle(node.id.as_str(), &self.nodes) {
                blocked.insert(node.id.clone());
                structural_issues.push(LibraryIssue::Cycle);
            }
        }

        for issue in structural_issues {
            self.record_issue(issue);
        }

        for node in &self.nodes {
            if blocked.contains(&node.id) {
                continue;
            }
            match node.parent_id.as_deref() {
                Some(parent_id) if known_ids.contains(parent_id) => self
                    .children
                    .entry(parent_id.to_owned())
                    .or_default()
                    .push(node.id.clone()),
                _ => self.root_ids.push(node.id.clone()),
            }
        }

        let nodes = &self.nodes;
        self.root_ids
            .sort_by(|left, right| compare_nodes(nodes, left, right));
        for child_ids in self.children.values_mut() {
            child_ids.sort_by(|left, right| compare_nodes(nodes, left, right));
        }
    }

    fn record_issue(&mut self, issue: LibraryIssue) {
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
    }
}

/// App-owned snapshot populated only by explicit Library refreshes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AtlasLibrarySnapshot {
    hierarchy: LibraryHierarchy,
}

impl AtlasLibrarySnapshot {
    #[must_use]
    pub const fn hierarchy(&self) -> &LibraryHierarchy {
        &self.hierarchy
    }

    pub(crate) fn replace_hierarchy(&mut self, hierarchy: LibraryHierarchy) {
        self.hierarchy = hierarchy;
    }
}

fn compare_nodes(nodes: &[LibraryNode], left: &str, right: &str) -> std::cmp::Ordering {
    let Some(left) = nodes.iter().find(|node| node.id == left) else {
        return left.cmp(right);
    };
    let Some(right) = nodes.iter().find(|node| node.id == right) else {
        return left.id.as_str().cmp(right);
    };
    match (left.order.as_deref(), right.order.as_deref()) {
        (Some(left_order), Some(right_order)) => compare_decimal_order(left_order, right_order),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
    .then_with(|| natural_path_compare(&left.path, &right.path))
    .then_with(|| left.id.cmp(&right.id))
}

fn has_cycle(start: &str, nodes: &[LibraryNode]) -> bool {
    let mut visited = BTreeSet::new();
    let mut current = Some(start);
    while let Some(id) = current {
        if !visited.insert(id) {
            return true;
        }
        current = nodes
            .iter()
            .find(|node| node.id == id)
            .and_then(|node| node.parent_id.as_deref())
            .filter(|parent_id| is_atlas_id(parent_id));
    }
    false
}

fn is_atlas_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty() && bytes.len() <= LIBRARY_ID_MAX_BYTES
}

fn bounded_title(value: &str) -> String {
    if value.len() <= LIBRARY_TITLE_MAX_BYTES {
        return value.to_owned();
    }
    let mut end = LIBRARY_TITLE_MAX_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

/// Atlas paths are Unicode, whereas the ESP32 does not carry ICU. Preserve a
/// deliberately small, auditable comparison domain that covers ASCII and the
/// NFC Latin letters whose base folds are implemented below. Anything outside
/// it, or beyond the exact retained byte bound, is withheld from this bounded
/// Library snapshot rather than being sorted with a lossy approximation.
fn is_supported_order_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= LIBRARY_PATH_MAX_BYTES
        && value.chars().all(is_supported_order_char)
}

const fn is_supported_order_char(character: char) -> bool {
    character.is_ascii()
        || matches!(
            character,
            'á' | 'à'
                | 'ä'
                | 'â'
                | 'ã'
                | 'å'
                | 'Á'
                | 'À'
                | 'Ä'
                | 'Â'
                | 'Ã'
                | 'Å'
                | 'é'
                | 'è'
                | 'ë'
                | 'ê'
                | 'É'
                | 'È'
                | 'Ë'
                | 'Ê'
                | 'í'
                | 'ì'
                | 'ï'
                | 'î'
                | 'Í'
                | 'Ì'
                | 'Ï'
                | 'Î'
                | 'ó'
                | 'ò'
                | 'ö'
                | 'ô'
                | 'õ'
                | 'ø'
                | 'Ó'
                | 'Ò'
                | 'Ö'
                | 'Ô'
                | 'Õ'
                | 'Ø'
                | 'ú'
                | 'ù'
                | 'ü'
                | 'û'
                | 'Ú'
                | 'Ù'
                | 'Ü'
                | 'Û'
                | 'ñ'
                | 'Ñ'
                | 'ç'
                | 'Ç'
        )
}

fn is_canonical_order(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= LIBRARY_ORDER_MAX_BYTES
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn compare_decimal_order(left: &str, right: &str) -> std::cmp::Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

/// Small, allocation-free approximation of `localeCompare('en', { numeric:
/// true, sensitivity: 'base' })` for bounded paths. Decimal runs compare by
/// numeric magnitude without integer conversion; common accented Latin input
/// folds to its base character before the stable-ID final tie-breaker.
fn natural_path_compare(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut left_chars = left.chars().peekable();
    let mut right_chars = right.chars().peekable();
    loop {
        match (left_chars.peek().copied(), right_chars.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left_char), Some(right_char))
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() =>
            {
                let left_digits: String = left_chars
                    .by_ref()
                    .take_while(char::is_ascii_digit)
                    .collect();
                let right_digits: String = right_chars
                    .by_ref()
                    .take_while(char::is_ascii_digit)
                    .collect();
                let left_number = left_digits.trim_start_matches('0');
                let right_number = right_digits.trim_start_matches('0');
                let comparison = left_number
                    .len()
                    .cmp(&right_number.len())
                    .then_with(|| left_number.cmp(right_number));
                if comparison != Ordering::Equal {
                    return comparison;
                }
            }
            (Some(left_char), Some(right_char)) => {
                left_chars.next();
                right_chars.next();
                let comparison = fold_sort_char(left_char).cmp(&fold_sort_char(right_char));
                if comparison != Ordering::Equal {
                    return comparison;
                }
            }
        }
    }
}

fn fold_sort_char(character: char) -> char {
    match character {
        'a'..='z' => character,
        'A'..='Z' => character.to_ascii_lowercase(),
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' | 'Å' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'ø' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' | 'Ø' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        other => other,
    }
}
