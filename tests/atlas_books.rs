use waveshare_epd397_rust_app::{
    app::{router::AtlasRoute, AppState},
    atlas_books::{BooksConnection, BooksView},
    atlas_client::{AtlasClient, MockAtlasTransport, MockTransportOutcome, TransportRequest},
    buttons::ButtonEvent,
};

const ID: &str = "book_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn books_home_flow_fetches_safe_blocks_and_pages_locally_without_sd() {
    let list = format!(
        r#"{{"items":[{{"id":"{ID}","title":"El país català","authors":["Mercè"],"language":"ca","byteSize":10,"importStatus":"ready"}}],"nextCursor":null}}"#
    );
    let manifest = format!(
        r#"{{"book":{{"id":"{ID}","title":"El país català","authors":["Mercè"],"language":"ca","byteSize":10,"importStatus":"ready"}},"spine":[{{"index":0,"label":"Capítol u","blockCount":1,"textBytes":60}}],"toc":[{{"label":"Capítol u","spineItem":0,"block":0}}]}}"#
    );
    let progress = "null";
    let bookmarks = r#"{"items":[]}"#;
    let segment = format!(
        r#"{{"bookId":"{ID}","spineItem":0,"cursor":null,"nextCursor":null,"blocks":[{{"index":0,"kind":"paragraph","text":"Hola, món: à é í ó ú ü ñ ç ¿ ¡"}}]}}"#
    );
    let mut transport = MockAtlasTransport::default();
    for response in [list, manifest, progress.into(), bookmarks.into(), segment] {
        transport.push_outcome(MockTransportOutcome::response(200, response));
    }
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();
    state.home_selected = 1;

    state.apply(ButtonEvent::Select);
    assert_eq!(state.atlas_route(), AtlasRoute::Books);
    assert!(state.has_pending_atlas_request());
    state.consume_atlas_requests(&mut client);
    assert_eq!(state.atlas_books.connection, BooksConnection::Connected);
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests(&mut client);
    assert_eq!(state.atlas_books.view, BooksView::Detail);
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests(&mut client);
    assert_eq!(state.atlas_books.view, BooksView::Reader);
    assert!(state.atlas_books.current_page.is_some());
    assert!(!state.storage.mounted);
    assert!(matches!(
        client.transport().requests()[4],
        TransportRequest::GetBookContent { .. }
    ));
}

#[test]
fn books_403_is_a_scope_upgrade_prompt_not_a_pairing_reset() {
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::forbidden());
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();
    state.home_selected = 1;
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests(&mut client);
    assert_eq!(
        state.atlas_books.connection,
        BooksConnection::RePairRequired
    );
    assert_eq!(
        state.atlas_books.feedback,
        Some("Re-pair device to enable Books")
    );
}

#[test]
fn books_back_tracks_the_local_reader_hierarchy_before_home() {
    let mut state = AppState::default();
    state.home_selected = 1;
    state.apply(ButtonEvent::Select);
    state.atlas_books.view = BooksView::Detail;
    state.back();
    assert_eq!(state.atlas_books.view, BooksView::List);
    assert_eq!(state.atlas_route(), AtlasRoute::Books);

    state.atlas_books.view = BooksView::Toc;
    state.back();
    assert_eq!(state.atlas_books.view, BooksView::Detail);
    assert_eq!(state.atlas_route(), AtlasRoute::Books);

    state.back();
    assert_eq!(state.atlas_books.view, BooksView::List);
    state.back();
    assert_eq!(state.atlas_route(), AtlasRoute::Home);
}
