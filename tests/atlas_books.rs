use waveshare_epd397_rust_app::atlas_dto::BOOK_COVER_BITMAP_BYTES;
use waveshare_epd397_rust_app::{
    app::{router::AtlasRoute, AppState},
    atlas_book_store::AtlasBookStore,
    atlas_books::{BooksConnection, BooksView},
    atlas_cache::AtlasCacheRepository,
    atlas_client::{AtlasClient, MockAtlasTransport, MockTransportOutcome, TransportRequest},
    atlas_storage::AtlasStorage,
    buttons::ButtonEvent,
};

const ID: &str = "book_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn books_list() -> String {
    format!(
        r#"{{"items":[{{"id":"{ID}","title":"El país català","authors":["Mercè"],"language":"ca","byteSize":10,"importStatus":"ready"}}],"nextCursor":null}}"#
    )
}

#[test]
fn books_reopen_from_bounded_sd_cache_after_a_cold_offline_start() {
    let cache_root = std::env::temp_dir().join(format!(
        "atlas-books-offline-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let cache = AtlasCacheRepository::new(AtlasStorage::new(&cache_root).unwrap());
    let mut online_transport = MockAtlasTransport::default();
    for response in [
        books_list(),
        two_chapter_manifest(),
        "null".into(),
        r#"{"items":[]}"#.into(),
        remote_segment(0, 0, 24),
    ] {
        online_transport.push_outcome(MockTransportOutcome::response(200, response));
    }
    let mut online_client = AtlasClient::new(online_transport);
    let mut online_state = AppState::default();
    online_state.home_selected = 1;
    online_state.apply(ButtonEvent::Select);
    online_state.consume_atlas_requests_with_cache(&mut online_client, Some(&cache));
    online_state.apply(ButtonEvent::Select);
    online_state.consume_atlas_requests_with_cache(&mut online_client, Some(&cache));
    online_state.apply(ButtonEvent::Select);
    online_state.consume_atlas_requests_with_cache(&mut online_client, Some(&cache));
    assert!(online_state.atlas_books.current_page.is_some());

    let mut offline_transport = MockAtlasTransport::default();
    for _ in 0..3 {
        offline_transport.push_outcome(MockTransportOutcome::offline());
    }
    let mut offline_client = AtlasClient::new(offline_transport);
    let mut offline_state = AppState::default();
    offline_state.home_selected = 1;
    offline_state.apply(ButtonEvent::Select);
    offline_state.consume_atlas_requests_with_cache(&mut offline_client, Some(&cache));
    assert_eq!(offline_state.atlas_books.books.len(), 1);
    assert_eq!(
        offline_state.atlas_books.connection,
        BooksConnection::Offline
    );
    offline_state.apply(ButtonEvent::Select);
    offline_state.consume_atlas_requests_with_cache(&mut offline_client, Some(&cache));
    assert_eq!(offline_state.atlas_books.view, BooksView::Detail);
    offline_state.apply(ButtonEvent::Select);
    offline_state.consume_atlas_requests_with_cache(&mut offline_client, Some(&cache));
    assert_eq!(offline_state.atlas_books.view, BooksView::Reader);
    assert!(offline_state.atlas_books.current_page.is_some());
    assert_eq!(
        offline_state.atlas_books.feedback,
        Some("Offline — saved copy")
    );
    std::fs::remove_dir_all(cache_root).unwrap();
}

#[test]
fn books_auto_sync_every_segment_and_reopen_the_complete_book_offline() {
    let root = std::env::temp_dir().join(format!(
        "atlas-books-complete-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = AtlasBookStore::new(&root).unwrap();
    let mut transport = MockAtlasTransport::default();
    for response in [
        books_list(),
        two_chapter_manifest(),
        remote_segment(0, 0, 24),
        remote_segment(0, 24, 1),
        remote_segment(1, 0, 2),
    ] {
        transport.push_outcome(MockTransportOutcome::response(200, response));
    }
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();
    state.home_selected = 1;
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests_with_stores(&mut client, None, Some(&store));
    while state.atlas_books.has_background_sync() {
        state.consume_atlas_requests_with_stores(&mut client, None, Some(&store));
    }
    assert_eq!(store.offline_list().unwrap().items.len(), 1);
    assert_eq!(state.atlas_books.feedback, Some("Available offline"));

    let mut offline = AppState::default();
    offline.hydrate_atlas_book_store(&store);
    offline.home_selected = 1;
    offline.apply(ButtonEvent::Select);
    let mut offline_transport = MockAtlasTransport::default();
    offline_transport.push_outcome(MockTransportOutcome::offline());
    offline_transport.push_outcome(MockTransportOutcome::offline());
    let mut offline_client = AtlasClient::new(offline_transport);
    offline.apply(ButtonEvent::Select);
    offline.consume_atlas_requests_with_stores(&mut offline_client, None, Some(&store));
    assert_eq!(offline.atlas_books.view, BooksView::Detail);
    offline.apply(ButtonEvent::Select);
    offline.consume_atlas_requests_with_stores(&mut offline_client, None, Some(&store));
    assert_eq!(offline.atlas_books.view, BooksView::Reader);
    assert!(offline.atlas_books.current_page.is_some());
    assert_eq!(
        offline.atlas_books.feedback,
        Some("Offline — downloaded book")
    );
    std::fs::remove_dir_all(root).unwrap();
}

fn two_chapter_manifest() -> String {
    format!(
        r#"{{"book":{{"id":"{ID}","title":"El país català","authors":["Mercè"],"language":"ca","byteSize":10,"importStatus":"ready"}},"spine":[{{"index":0,"label":"Capítol u","blockCount":25,"textBytes":400}},{{"index":1,"label":"Capítol dos","blockCount":2,"textBytes":40}}],"toc":[{{"label":"Capítol u","spineItem":0,"block":0}},{{"label":"Capítol dos","spineItem":1,"block":0}}]}}"#
    )
}

fn remote_segment(spine_item: u16, first_block: u16, count: u16) -> String {
    let blocks = (first_block..first_block.saturating_add(count))
        .map(|index| {
            let kind = if index == 0 { "heading" } else { "paragraph" };
            format!(r#"{{"index":{index},"kind":"{kind}","text":"bloc {index}"}}"#)
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"bookId":"{ID}","spineItem":{spine_item},"cursor":null,"nextCursor":null,"blocks":[{blocks}]}}"#
    )
}

fn open_reader(state: &mut AppState, client: &mut AtlasClient<MockAtlasTransport>) {
    state.home_selected = 1;
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests(client);
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests(client);
    assert_eq!(state.atlas_books.view, BooksView::Detail);
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests(client);
    assert_eq!(state.atlas_books.view, BooksView::Reader);
}

#[test]
fn books_home_flow_fetches_safe_blocks_and_pages_locally_without_sd() {
    let list = books_list();
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
fn books_cross_segments_and_spines_with_bounded_history_and_no_duplicate_requests() {
    let mut transport = MockAtlasTransport::default();
    for response in [
        books_list(),
        two_chapter_manifest(),
        "null".into(),
        r#"{"items":[]}"#.into(),
        remote_segment(0, 0, 24),
        remote_segment(0, 24, 1),
        remote_segment(1, 0, 2),
    ] {
        transport.push_outcome(MockTransportOutcome::response(200, response));
    }
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();
    open_reader(&mut state, &mut client);
    assert_eq!(
        state
            .atlas_books
            .current_page
            .as_ref()
            .unwrap()
            .anchor
            .block,
        0
    );

    // The first page consumes the complete shared physical viewport.
    let page_blocks = state.reader.preferences.layout().lines_per_page as u16;
    state.apply(ButtonEvent::Down);
    assert_eq!(
        state
            .atlas_books
            .current_page
            .as_ref()
            .unwrap()
            .anchor
            .block,
        page_blocks
    );
    state.apply(ButtonEvent::Down);
    assert!(state.atlas_books.has_pending_request());
    state.consume_atlas_requests(&mut client);
    assert_eq!(
        state
            .atlas_books
            .current_page
            .as_ref()
            .unwrap()
            .anchor
            .block,
        24
    );

    // Crossing the spine asks precisely once for the destination block.
    state.apply(ButtonEvent::Down);
    state.consume_atlas_requests(&mut client);
    assert_eq!(
        state
            .atlas_books
            .current_page
            .as_ref()
            .unwrap()
            .anchor
            .spine_item,
        1
    );
    let content_requests = client
        .transport()
        .requests()
        .iter()
        .filter_map(|request| match request {
            TransportRequest::GetBookContent {
                spine_item, block, ..
            } => Some((*spine_item, *block)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        content_requests,
        vec![(0, Some(0)), (0, Some(24)), (1, Some(0))]
    );

    // Up uses only the bounded page history, retaining the exact prior anchor.
    let request_count = client.transport().requests().len();
    state.apply(ButtonEvent::Up);
    let page = state.atlas_books.current_page.as_ref().unwrap();
    assert_eq!((page.anchor.spine_item, page.anchor.block), (0, 24));
    assert_eq!(client.transport().requests().len(), request_count);
}

#[test]
fn failed_boundary_fetch_leaves_the_displayed_page_and_history_intact() {
    let mut transport = MockAtlasTransport::default();
    for response in [
        books_list(),
        two_chapter_manifest(),
        "null".into(),
        r#"{"items":[]}"#.into(),
        remote_segment(0, 0, 24),
    ] {
        transport.push_outcome(MockTransportOutcome::response(200, response));
    }
    transport.push_outcome(MockTransportOutcome::offline());
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();
    open_reader(&mut state, &mut client);
    let page_blocks = state.reader.preferences.layout().lines_per_page as u16;
    state.apply(ButtonEvent::Down);
    assert_eq!(
        state
            .atlas_books
            .current_page
            .as_ref()
            .unwrap()
            .anchor
            .block,
        page_blocks
    );
    state.apply(ButtonEvent::Down);
    state.consume_atlas_requests(&mut client);
    assert_eq!(
        state
            .atlas_books
            .current_page
            .as_ref()
            .unwrap()
            .anchor
            .block,
        page_blocks
    );
    assert_eq!(state.atlas_books.connection, BooksConnection::Offline);
    assert_eq!(state.atlas_books.feedback, Some("Offline"));
}

#[test]
fn resume_and_bookmark_at_block_seventy_fetch_that_block_directly() {
    let progress = format!(
        r#"{{"bookId":"{ID}","anchor":{{"spineItem":0,"block":70,"characterOffset":6}},"percentage":90,"revision":1}}"#
    );
    let bookmarks = format!(
        r#"{{"items":[{{"id":"bm_1","bookId":"{ID}","anchor":{{"spineItem":0,"block":70,"characterOffset":6}},"label":"Retomar"}}]}}"#
    );
    let mut transport = MockAtlasTransport::default();
    for response in [
        books_list(),
        two_chapter_manifest(),
        progress,
        bookmarks,
        remote_segment(0, 70, 2),
        remote_segment(0, 70, 2),
    ] {
        transport.push_outcome(MockTransportOutcome::response(200, response));
    }
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();
    open_reader(&mut state, &mut client);
    let page = state.atlas_books.current_page.as_ref().unwrap();
    assert_eq!((page.anchor.block, page.anchor.character_offset), (70, 6));
    assert!(matches!(
        client.transport().requests()[4],
        TransportRequest::GetBookContent {
            spine_item: 0,
            block: Some(70),
            cursor: None,
            ..
        }
    ));

    state.atlas_books.leave_reader();
    state.atlas_books.view = BooksView::Bookmarks;
    state.atlas_books.selected = 0;
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests(&mut client);
    assert_eq!(
        client.transport().requests().len(),
        5,
        "the retained bounded segment reopens the bookmark without a duplicate request"
    );
    assert_eq!(
        state
            .atlas_books
            .current_page
            .as_ref()
            .unwrap()
            .anchor
            .block,
        70
    );
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
    assert_eq!(state.atlas_books.feedback, Some("Authorization required"));
}

#[test]
fn opening_a_covered_book_fetches_and_persists_the_bounded_eink_cover() {
    let cache_root = std::env::temp_dir().join(format!(
        "atlas-book-cover-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let cache = AtlasCacheRepository::new(AtlasStorage::new(&cache_root).unwrap());
    let list = format!(
        r#"{{"items":[{{"id":"{ID}","title":"Covered","authors":["Author"],"language":"en","byteSize":10,"importStatus":"ready","coverUrl":"/api/v1/books/{ID}/cover"}}],"nextCursor":null}}"#
    );
    let manifest = format!(
        r#"{{"book":{{"id":"{ID}","title":"Covered","authors":["Author"],"language":"en","byteSize":10,"importStatus":"ready","coverUrl":"/api/v1/books/{ID}/cover"}},"spine":[{{"index":0,"label":"One","blockCount":1,"textBytes":1}}],"toc":[]}}"#
    );
    let mut pbm = b"P4\n104 142\n".to_vec();
    pbm.extend(vec![0xaa; BOOK_COVER_BITMAP_BYTES]);
    let mut transport = MockAtlasTransport::default();
    for outcome in [
        MockTransportOutcome::response(200, list),
        MockTransportOutcome::response(200, manifest),
        MockTransportOutcome::response(200, pbm),
        MockTransportOutcome::response(200, "null"),
        MockTransportOutcome::response(200, r#"{"items":[]}"#),
    ] {
        transport.push_outcome(outcome);
    }
    let mut client = AtlasClient::new(transport);
    let mut state = AppState::default();
    state.home_selected = 1;
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests_with_cache(&mut client, Some(&cache));
    state.apply(ButtonEvent::Select);
    state.consume_atlas_requests_with_cache(&mut client, Some(&cache));

    assert_eq!(
        state.atlas_books.cover_for(ID).unwrap().pixels.len(),
        BOOK_COVER_BITMAP_BYTES
    );
    assert_eq!(
        cache.offline_book_cover(ID).value.unwrap().pixels.len(),
        BOOK_COVER_BITMAP_BYTES
    );
    assert!(matches!(
        client.transport().requests()[2],
        TransportRequest::GetBookCover { .. }
    ));
    std::fs::remove_dir_all(cache_root).unwrap();
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

#[test]
fn atlas_home_shortcut_consumes_all_books_levels_but_short_back_consumes_one() {
    let mut state = AppState::default();
    state.home_selected = 1;
    state.apply(ButtonEvent::Select);
    state.atlas_books.view = BooksView::Toc;

    assert!(state.apply_hierarchical_back());
    assert_eq!(state.atlas_books.view, BooksView::Detail);
    assert_eq!(state.atlas_route(), AtlasRoute::Books);

    assert!(state.apply_atlas_home_shortcut());
    assert_eq!(state.atlas_books.view, BooksView::List);
    assert_eq!(state.atlas_route(), AtlasRoute::Home);
}
