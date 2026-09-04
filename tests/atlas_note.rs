use waveshare_epd397_rust_app::{
    app::{
        display::{DisplayPreferences, UiFontFamily, UiFontSize},
        render_current_screen,
        router::{AtlasNavigationSurface, AtlasNoteOrigin, AtlasRoute},
        state::AppState,
    },
    atlas_client::{AtlasClient, MockAtlasTransport, MockTransportOutcome, TransportRequest},
    atlas_note::{
        AtlasNoteError, AtlasNoteStatus, MAX_ATLAS_NOTE_BODY_BYTES, MAX_ATLAS_NOTE_TITLE_BYTES,
        MAX_RECENT_ATLAS_NOTES,
    },
    buttons::ButtonEvent,
    framebuffer::FrameBuffer,
    orientation::DisplayOrientation,
};

const NOTE_ID: &str = "11111111-1111-4111-8111-111111111111";

fn note_json(id: &str, title: &str, body: &str) -> String {
    serde_json::json!({
        "id": id,
        "title": title,
        "revision": "r1",
        "body": body,
        "parentId": null,
        "order": null
    })
    .to_string()
}

fn client_with(outcome: MockTransportOutcome) -> AtlasClient<MockAtlasTransport> {
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(outcome);
    AtlasClient::new(transport)
}

#[test]
fn note_load_uses_selected_id_and_exposes_loading_then_loaded() {
    let mut state = AppState::default();
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Home));
    assert_eq!(state.atlas_note.status(), AtlasNoteStatus::Loading);
    assert_eq!(state.atlas_note.selected_id(), Some(NOTE_ID));
    assert_eq!(state.atlas_note.origin(), Some(AtlasNoteOrigin::Home));

    let mut client = client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Morning", "# Hello\n\nBody"),
    ));
    state.load_atlas_note(&mut client);

    assert_eq!(state.atlas_note.status(), AtlasNoteStatus::Loaded);
    assert_eq!(state.atlas_note.document().unwrap().title(), "Morning");
    assert_eq!(
        state.atlas_note.document().unwrap().body(),
        "# Hello\n\nBody"
    );
    assert_eq!(state.atlas_note.recent().len(), 1);
    assert_eq!(
        client.transport().requests(),
        [TransportRequest::GetNote { id: NOTE_ID.into() }]
    );
}

#[test]
fn offline_refresh_uses_bounded_recent_document_without_blank_screen() {
    let mut state = AppState::default();
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Library));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Cached", "cached body"),
    )));
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Library));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::offline()));

    assert_eq!(state.atlas_note.status(), AtlasNoteStatus::OfflineCached);
    assert_eq!(state.atlas_note.document().unwrap().body(), "cached body");
}

#[test]
fn note_errors_are_classified_and_previous_document_is_retained() {
    let mut state = AppState::default();
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Views));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Useful", "still here"),
    )));
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Views));

    state.load_atlas_note(&mut client_with(MockTransportOutcome::not_found()));
    assert_eq!(
        state.atlas_note.status(),
        AtlasNoteStatus::Error(AtlasNoteError::NotFound)
    );
    assert_eq!(state.atlas_note.document().unwrap().body(), "still here");

    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Views));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::malformed()));
    assert_eq!(
        state.atlas_note.status(),
        AtlasNoteStatus::Error(AtlasNoteError::MalformedPayload)
    );
}

#[test]
fn note_http_transport_and_parse_failures_are_all_explicit() {
    for (outcome, expected) in [
        (MockTransportOutcome::not_found(), AtlasNoteError::NotFound),
        (
            MockTransportOutcome::unauthorized(),
            AtlasNoteError::Unauthorized,
        ),
        (MockTransportOutcome::forbidden(), AtlasNoteError::Forbidden),
        (
            MockTransportOutcome::unavailable(),
            AtlasNoteError::Unavailable,
        ),
        (MockTransportOutcome::timeout(), AtlasNoteError::Timeout),
        (MockTransportOutcome::offline(), AtlasNoteError::Offline),
        (
            MockTransportOutcome::malformed(),
            AtlasNoteError::MalformedPayload,
        ),
        (MockTransportOutcome::oversized(), AtlasNoteError::Oversized),
    ] {
        let mut state = AppState::default();
        assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Home));
        state.load_atlas_note(&mut client_with(outcome));
        assert_eq!(state.atlas_note.status(), AtlasNoteStatus::Error(expected));
        assert!(state.atlas_note.document().is_none());
    }
}

#[test]
fn empty_body_unicode_and_oversized_reader_fields_are_safe() {
    let mut state = AppState::default();
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Home));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Caf\u{00e9} \u{1f4da}", ""),
    )));
    assert_eq!(state.atlas_note.status(), AtlasNoteStatus::Loaded);
    assert_eq!(state.atlas_note.document().unwrap().body(), "");
    assert_eq!(
        state.atlas_note.document().unwrap().title(),
        "Caf\u{00e9} \u{1f4da}"
    );

    let oversized_title = "t".repeat(MAX_ATLAS_NOTE_TITLE_BYTES + 1);
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Home));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, &oversized_title, "body"),
    )));
    assert_eq!(
        state.atlas_note.status(),
        AtlasNoteStatus::Error(AtlasNoteError::Oversized)
    );
    assert_eq!(state.atlas_note.document().unwrap().body(), "");

    let oversized_body = "b".repeat(MAX_ATLAS_NOTE_BODY_BYTES + 1);
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Home));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Title", &oversized_body),
    )));
    assert_eq!(
        state.atlas_note.status(),
        AtlasNoteStatus::Error(AtlasNoteError::Oversized)
    );
}

#[test]
fn selecting_a_different_note_never_displays_an_unrelated_document() {
    let other_id = "22222222-2222-4222-8222-222222222222";
    let mut state = AppState::default();
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Library));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "First", "first body"),
    )));

    assert!(state.begin_atlas_note(other_id, AtlasNoteOrigin::Home));
    assert_eq!(state.atlas_note.status(), AtlasNoteStatus::Loading);
    assert!(state.atlas_note.document().is_none());
    state.load_atlas_note(&mut client_with(MockTransportOutcome::offline()));
    assert_eq!(
        state.atlas_note.status(),
        AtlasNoteStatus::Error(AtlasNoteError::Offline)
    );
    assert!(state.atlas_note.document().is_none());
}

#[test]
fn route_only_selection_cannot_reopen_a_prior_note_without_an_id() {
    let mut state = AppState::default();
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Library));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Prior", "prior body"),
    )));

    state
        .router
        .navigate_atlas_to(AtlasNavigationSurface::Library);
    state.apply(waveshare_epd397_rust_app::buttons::ButtonEvent::Select);

    assert_eq!(state.atlas_route(), AtlasRoute::Library);
    assert_eq!(state.atlas_note.selected_id(), Some(NOTE_ID));
    assert_eq!(state.atlas_note.document().unwrap().body(), "prior body");
}

#[test]
fn recent_notes_are_bounded_and_origin_survives_back_including_home() {
    let mut state = AppState::default();
    for index in 0..=MAX_RECENT_ATLAS_NOTES {
        let id = format!("0000000{index}-0000-4000-8000-000000000000");
        assert!(state.begin_atlas_note(&id, AtlasNoteOrigin::Home));
        state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
            200,
            note_json(&id, "Title", "body"),
        )));
    }
    assert_eq!(state.atlas_note.recent().len(), MAX_RECENT_ATLAS_NOTES);
    assert_eq!(state.atlas_note.origin(), Some(AtlasNoteOrigin::Home));
    assert_eq!(state.atlas_route(), AtlasRoute::Note);
    state.back();
    assert_eq!(state.atlas_route(), AtlasRoute::Home);
}

#[test]
fn note_bounds_and_identity_fail_safely() {
    let mut state = AppState::default();
    assert!(!state.begin_atlas_note("", AtlasNoteOrigin::Home));
    assert_eq!(
        state.atlas_note.status(),
        AtlasNoteStatus::Error(AtlasNoteError::InvalidId)
    );

    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Home));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json("22222222-2222-4222-8222-222222222222", "Wrong", "body"),
    )));
    assert_eq!(
        state.atlas_note.status(),
        AtlasNoteStatus::Error(AtlasNoteError::MalformedPayload)
    );
    assert!(state.atlas_note.document().is_none());
}

#[test]
fn note_page_turning_uses_prebuilt_pages_without_changing_origin_or_loading_state() {
    let mut state = AppState::default();
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Library));
    let body = (0..120)
        .map(|index| format!("word{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Paged", &body),
    )));

    assert!(state.atlas_note.page_count() > 1);
    assert_eq!(state.atlas_note.page_index(), 0);
    state.apply(ButtonEvent::Down);
    assert_eq!(state.atlas_note.page_index(), 1);
    assert_eq!(state.atlas_note.origin(), Some(AtlasNoteOrigin::Library));
    assert_eq!(state.atlas_note.status(), AtlasNoteStatus::Loaded);
    state.apply(ButtonEvent::Up);
    assert_eq!(state.atlas_note.page_index(), 0);
}

#[test]
fn reopening_the_same_cached_note_preserves_the_current_page() {
    let mut state = AppState::default();
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Library));
    let body = (0..120)
        .map(|index| format!("word{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Paged", &body),
    )));
    state.apply(ButtonEvent::Down);
    assert_eq!(state.atlas_note.page_index(), 1);

    state.back();
    assert_eq!(state.atlas_route(), AtlasRoute::Library);
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Library));

    assert_eq!(state.atlas_note.page_index(), 1);

    assert!(state.begin_atlas_note(
        "22222222-2222-4222-8222-222222222222",
        AtlasNoteOrigin::Library
    ));
    assert_eq!(state.atlas_note.page_index(), 0);
}

#[test]
fn successful_shorter_reload_clamps_the_cached_page_cursor() {
    let mut state = AppState::default();
    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Library));
    let old_body = (0..120)
        .map(|index| format!("word{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Paged", &old_body),
    )));
    state.apply(ButtonEvent::Down);
    assert_eq!(state.atlas_note.page_index(), 1);

    assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Library));
    state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
        200,
        note_json(NOTE_ID, "Short", "one page"),
    )));

    assert_eq!(state.atlas_note.page_count(), 1);
    assert_eq!(state.atlas_note.page_index(), 0);
    assert!(state.atlas_note.current_page().is_some());
}

#[test]
fn note_heading_ink_is_present_in_the_safe_viewport_for_every_font_profile() {
    for font_family in [UiFontFamily::Inter, UiFontFamily::AtkinsonHyperlegible] {
        for font_size in [UiFontSize::Compact, UiFontSize::Standard, UiFontSize::Large] {
            let mut state = AppState::default();
            state.display = DisplayPreferences {
                font_family,
                font_size,
            };
            assert!(state.begin_atlas_note(NOTE_ID, AtlasNoteOrigin::Home));
            state.load_atlas_note(&mut client_with(MockTransportOutcome::response(
                200,
                note_json(NOTE_ID, "Title", "# Heading\n\nBody"),
            )));

            let mut frame = FrameBuffer::new_white();
            render_current_screen(&mut frame, &state).unwrap();
            let mut heading_ink = false;
            for y in 206..226 {
                for x in 22..180 {
                    let native = DisplayOrientation::Portrait
                        .map_logical_to_native(embedded_graphics::prelude::Point::new(x, y))
                        .unwrap();
                    heading_ink |= frame.is_black(native) == Some(true);
                }
            }
            assert!(heading_ink, "missing heading ink for {font_family:?}/{font_size:?}");
        }
    }
}
