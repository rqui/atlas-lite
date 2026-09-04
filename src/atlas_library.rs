//! Bounded, defensive Atlas Library hierarchy derived from note summaries.

use std::collections::{BTreeMap, BTreeSet};

use crate::atlas_dto::NoteSummaryPage;

/// Absolute number of summaries retained for the Library on this device.
pub const LIBRARY_NODE_LIMIT: usize = 64;
/// Maximum UTF-8 bytes retained for one rendered Library title.
pub const LIBRARY_TITLE_MAX_BYTES: usize = 96;
/// Maximum bytes retained for one Atlas sibling-order key.
pub const LIBRARY_ORDER_MAX_BYTES: usize = 64;
/// The bounded number of summaries requested from one Atlas page.
pub const LIBRARY_PAGE_SIZE: usize = 16;
/// The bounded number of Atlas pages an explicit Library refresh may request.
pub const LIBRARY_PAGE_LIMIT: usize = 4;

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
                    Some(order) if order.len() > LIBRARY_ORDER_MAX_BYTES => {
                        hierarchy.record_issue(LibraryIssue::OrderTooLong);
                        None
                    }
                    Some(order) => Some(order.to_owned()),
                    None => None,
                };

                hierarchy.nodes.push(LibraryNode {
                    id: id.to_owned(),
                    parent_id,
                    order,
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
                    blocked.insert(node.id.clone());
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
                Some(parent_id) => self
                    .children
                    .entry(parent_id.to_owned())
                    .or_default()
                    .push(node.id.clone()),
                None => self.root_ids.push(node.id.clone()),
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
        (Some(left_order), Some(right_order)) => left_order.cmp(right_order),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
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
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) && *byte == b'-'
                || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
        })
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
