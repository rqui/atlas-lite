use waveshare_epd397_rust_app::{
    app::screens::atlas_library::atlas_library_content,
    app::{router::AtlasRoute, AppState},
    atlas_client::{AtlasClient, MockAtlasTransport, MockTransportOutcome, TransportRequest},
    atlas_dto::{AtlasNoteSummary, NoteState, NoteSummaryPage},
    atlas_library::{
        LibraryCompleteness, LibraryHierarchy, LibraryIssue, LIBRARY_ID_MAX_BYTES,
        LIBRARY_NODE_LIMIT, LIBRARY_ORDER_MAX_BYTES, LIBRARY_TITLE_MAX_BYTES,
    },
    buttons::ButtonEvent,
};

fn summary(
    id: Option<&str>,
    parent_id: Option<&str>,
    order: Option<&str>,
    title: &str,
) -> AtlasNoteSummary {
    AtlasNoteSummary {
        id: id.map(str::to_owned),
        path: "not-retained.md".into(),
        title: title.into(),
        state: NoteState::Managed,
        revision: "r1".into(),
        parent_id: parent_id.map(str::to_owned),
        order: order.map(str::to_owned),
    }
}

fn page(items: Vec<AtlasNoteSummary>, next_cursor: Option<&str>) -> NoteSummaryPage {
    NoteSummaryPage {
        items,
        next_cursor: next_cursor.map(str::to_owned),
    }
}

#[test]
fn combines_multiple_pages_by_id_and_preserves_order_with_deterministic_ties() {
    let hierarchy = LibraryHierarchy::from_pages(&[
        page(
            vec![
                summary(
                    Some("11111111-1111-4111-8111-111111111111"),
                    None,
                    Some("b"),
                    "Second",
                ),
                summary(
                    Some("22222222-2222-4222-8222-222222222222"),
                    None,
                    Some("a"),
                    "First",
                ),
            ],
            Some("page-2"),
        ),
        page(
            vec![
                summary(
                    Some("33333333-3333-4333-8333-333333333333"),
                    None,
                    Some("b"),
                    "Tie",
                ),
                summary(
                    Some("44444444-4444-4444-8444-444444444444"),
                    None,
                    None,
                    "Unordered",
                ),
            ],
            None,
        ),
    ]);

    assert_eq!(hierarchy.completeness(), LibraryCompleteness::Complete);
    assert_eq!(
        hierarchy.root_ids(),
        [
            "22222222-2222-4222-8222-222222222222",
            "11111111-1111-4111-8111-111111111111",
            "33333333-3333-4333-8333-333333333333",
            "44444444-4444-4444-8444-444444444444",
        ]
    );
    assert_eq!(hierarchy.nodes()[0].title(), "Second");
}

#[test]
fn marks_a_remaining_cursor_as_incomplete_without_claiming_a_complete_vault() {
    let hierarchy = LibraryHierarchy::from_pages(&[page(
        vec![summary(
            Some("11111111-1111-4111-8111-111111111111"),
            None,
            None,
            "Only page",
        )],
        Some("more"),
    )]);

    assert_eq!(
        hierarchy.completeness(),
        LibraryCompleteness::CursorRemaining
    );
}

#[test]
fn records_invalid_structure_without_linking_or_traversing_it() {
    let hierarchy = LibraryHierarchy::from_pages(&[page(
        vec![
            summary(None, None, None, "Null"),
            summary(Some(""), None, None, "Invalid"),
            summary(
                Some("11111111-1111-4111-8111-111111111111"),
                None,
                None,
                "Parent",
            ),
            summary(
                Some("22222222-2222-4222-8222-222222222222"),
                Some("55555555-5555-4555-8555-555555555555"),
                None,
                "Orphan",
            ),
            summary(
                Some("66666666-6666-4666-8666-666666666666"),
                Some(""),
                None,
                "Bad parent",
            ),
            summary(Some("cycle/a"), Some("cycle/b"), None, "Cycle A"),
            summary(Some("cycle/b"), Some("cycle/a"), None, "Cycle B"),
            summary(
                Some("11111111-1111-4111-8111-111111111111"),
                None,
                None,
                "Duplicate",
            ),
        ],
        None,
    )]);

    assert_eq!(hierarchy.nodes().len(), 4);
    assert_eq!(
        hierarchy.root_ids(),
        ["11111111-1111-4111-8111-111111111111"]
    );
    for issue in [
        LibraryIssue::MissingId,
        LibraryIssue::InvalidId,
        LibraryIssue::MissingParent,
        LibraryIssue::InvalidParent,
        LibraryIssue::Cycle,
        LibraryIssue::DuplicateId,
    ] {
        assert!(hierarchy.issues().contains(&issue), "missing {issue:?}");
    }
}

#[test]
fn enforces_the_named_node_budget_without_allocating_an_unbounded_tree() {
    let items = (0..LIBRARY_NODE_LIMIT + 1)
        .map(|index| {
            summary(
                Some(&format!("00000000-0000-4000-8000-{index:012}")),
                None,
                None,
                "Bounded",
            )
        })
        .collect();
    let hierarchy = LibraryHierarchy::from_pages(&[page(items, None)]);

    assert_eq!(hierarchy.nodes().len(), LIBRARY_NODE_LIMIT);
    assert_eq!(
        hierarchy.completeness(),
        LibraryCompleteness::NodeBudgetReached
    );
    assert_eq!(hierarchy.issues(), [LibraryIssue::NodeBudgetReached]);
}

#[test]
fn explicit_library_refresh_uses_bounded_client_pages_and_exposes_partial_hierarchy() {
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(
        200,
        br#"{"items":[{"id":"11111111-1111-4111-8111-111111111111","path":"ignored.md","title":"Parent","state":"managed","revision":"r1","parentId":null,"order":"a"}],"nextCursor":"page-2"}"#,
    ));
    transport.push_outcome(MockTransportOutcome::response(
        200,
        br#"{"items":[{"id":"22222222-2222-4222-8222-222222222222","path":"ignored.md","title":"Child","state":"managed","revision":"r1","parentId":"11111111-1111-4111-8111-111111111111","order":"a"}],"nextCursor":"page-3"}"#,
    ));
    transport.push_outcome(MockTransportOutcome::response(
        200,
        br#"{"items":[],"nextCursor":"page-4"}"#,
    ));
    transport.push_outcome(MockTransportOutcome::response(
        200,
        br#"{"items":[],"nextCursor":"more"}"#,
    ));
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();

    state.refresh_atlas_library(&mut client);

    assert_eq!(
        state.atlas_library.hierarchy().root_ids(),
        ["11111111-1111-4111-8111-111111111111"]
    );
    assert_eq!(
        state
            .atlas_library
            .hierarchy()
            .child_ids("11111111-1111-4111-8111-111111111111"),
        ["22222222-2222-4222-8222-222222222222"]
    );
    assert_eq!(
        state.atlas_library.hierarchy().completeness(),
        LibraryCompleteness::CursorRemaining
    );
    assert_eq!(state.atlas_route(), AtlasRoute::Home);
    assert_eq!(
        client.transport().requests(),
        &[
            TransportRequest::ListNotes {
                cursor: None,
                limit: 16
            },
            TransportRequest::ListNotes {
                cursor: Some("page-2".into()),
                limit: 16
            },
            TransportRequest::ListNotes {
                cursor: Some("page-3".into()),
                limit: 16
            },
            TransportRequest::ListNotes {
                cursor: Some("page-4".into()),
                limit: 16
            },
        ]
    );
}

#[test]
fn library_select_opens_only_the_visible_hierarchy_id_not_the_summary_path() {
    let id = "11111111-1111-4111-8111-111111111111";
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(
        200,
        br#"{"items":[{"id":"11111111-1111-4111-8111-111111111111","path":"a/path-that-must-never-open.md","title":"Real identity","state":"managed","revision":"r1","parentId":null,"order":"a"}],"nextCursor":null}"#,
    ));
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();
    state.refresh_atlas_library(&mut client);
    state
        .router
        .navigate_atlas_to(waveshare_epd397_rust_app::app::router::AtlasNavigationSurface::Library);

    state.apply(ButtonEvent::Select);

    assert_eq!(state.atlas_route(), AtlasRoute::Note);
    assert_eq!(state.atlas_note.selected_id(), Some(id));
    assert_eq!(
        state.atlas_note.origin(),
        Some(waveshare_epd397_rust_app::app::router::AtlasNoteOrigin::Library)
    );
    assert_eq!(
        client.transport().requests().len(),
        1,
        "selection itself does not fetch"
    );
}

#[test]
fn library_with_no_valid_hierarchy_id_cannot_open_a_note() {
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(
        200,
        br#"{"items":[{"id":null,"path":"fabricated.md","title":"No identity","state":"managed","revision":"r1","parentId":null,"order":null}],"nextCursor":null}"#,
    ));
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();
    state.refresh_atlas_library(&mut client);
    state
        .router
        .navigate_atlas_to(waveshare_epd397_rust_app::app::router::AtlasNavigationSurface::Library);

    state.apply(ButtonEvent::Select);

    assert_eq!(state.atlas_route(), AtlasRoute::Library);
    assert_eq!(state.atlas_note.selected_id(), None);
    assert_eq!(client.transport().requests().len(), 1);
}

#[test]
fn library_renderer_content_uses_owned_hierarchy_and_labels_partial_results() {
    let hierarchy = LibraryHierarchy::from_pages(&[page(
        vec![
            summary(
                Some("11111111-1111-4111-8111-111111111111"),
                None,
                None,
                "Parent",
            ),
            summary(
                Some("22222222-2222-4222-8222-222222222222"),
                Some("11111111-1111-4111-8111-111111111111"),
                None,
                "Child",
            ),
        ],
        Some("more"),
    )]);

    let content = atlas_library_content(&hierarchy);

    assert_eq!(content.status(), "PARTIAL");
    assert_eq!(content.entries(), ["Parent", "  Child"]);
}

#[test]
fn bounds_retained_title_and_order_fields_without_retaining_oversized_payloads() {
    let hierarchy = LibraryHierarchy::from_pages(&[page(
        vec![summary(
            Some("11111111-1111-4111-8111-111111111111"),
            None,
            Some(&"z".repeat(LIBRARY_ORDER_MAX_BYTES + 1)),
            &"é".repeat(LIBRARY_TITLE_MAX_BYTES + 1),
        )],
        None,
    )]);

    assert!(hierarchy.nodes()[0].title().len() <= LIBRARY_TITLE_MAX_BYTES);
    assert_eq!(hierarchy.nodes()[0].order(), None);
    assert!(hierarchy.issues().contains(&LibraryIssue::OrderTooLong));
}

#[test]
fn rejects_an_invalid_parent_record_without_reserving_its_id_from_a_later_valid_record() {
    let hierarchy = LibraryHierarchy::from_pages(&[page(
        vec![
            summary(
                Some("11111111-1111-4111-8111-111111111111"),
                Some(""),
                None,
                "Rejected",
            ),
            summary(
                Some("11111111-1111-4111-8111-111111111111"),
                None,
                None,
                "Accepted",
            ),
        ],
        None,
    )]);

    assert_eq!(hierarchy.nodes().len(), 1);
    assert_eq!(hierarchy.nodes()[0].title(), "Accepted");
    assert!(hierarchy.issues().contains(&LibraryIssue::InvalidParent));
    assert!(!hierarchy.issues().contains(&LibraryIssue::DuplicateId));
}

#[test]
fn accepts_non_empty_opaque_ids_and_path_like_parent_ids() {
    let hierarchy = LibraryHierarchy::from_pages(&[page(
        vec![
            summary(Some("projects"), None, None, "Projects"),
            summary(Some("projects/atlas"), Some("projects"), None, "Atlas"),
        ],
        None,
    )]);

    assert_eq!(hierarchy.nodes().len(), 2);
    assert_eq!(hierarchy.root_ids(), ["projects"]);
    assert_eq!(hierarchy.child_ids("projects"), ["projects/atlas"]);
    assert!(hierarchy.issues().is_empty());
}

#[test]
fn rejects_empty_and_oversized_opaque_ids_and_parent_ids_safely() {
    let oversized = "x".repeat(LIBRARY_ID_MAX_BYTES + 1);
    let hierarchy = LibraryHierarchy::from_pages(&[page(
        vec![
            summary(Some(""), None, None, "Empty ID"),
            summary(Some(&oversized), None, None, "Oversized ID"),
            summary(Some("valid"), Some(""), None, "Empty parent"),
            summary(Some("valid-2"), Some(&oversized), None, "Oversized parent"),
        ],
        None,
    )]);

    assert!(hierarchy.nodes().is_empty());
    assert!(hierarchy.issues().contains(&LibraryIssue::InvalidId));
    assert!(hierarchy.issues().contains(&LibraryIssue::InvalidParent));
}
