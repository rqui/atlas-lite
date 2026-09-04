use waveshare_epd397_rust_app::atlas_dto::{
    parse_api_error, parse_note_document, parse_note_summary_page, parse_search_response,
    parse_view_result_page, parse_view_summaries, AtlasDtoError, ViewLayout, ViewStatus,
    MAX_RESPONSE_BODY_BYTES,
};

const NOTE_PAGE: &[u8] = br#"{
  "items": [{
    "id": "note-1",
    "path": "projects/atlas.md",
    "state": "managed",
    "title": "Atlas Lite",
    "icon": "book",
    "created": "2026-09-04T10:00:00.000Z",
    "updated": "2026-09-04T10:00:00.000Z",
    "revision": "abc123",
    "frontmatter": {"secret": {"nested": true}},
    "parentId": "projects",
    "order": "a"
  }],
  "nextCursor": "cursor-2",
  "serverExtension": true
}"#;

const NOTE_DOCUMENT: &[u8] = br##"{
  "id": "note-1",
  "path": "projects/atlas.md",
  "state": "managed",
  "title": "Atlas Lite",
  "created": "2026-09-04T10:00:00.000Z",
  "updated": "2026-09-04T10:00:00.000Z",
  "revision": "abc123",
  "frontmatter": {"tags": ["device"], "extra": {"ignored": true}},
  "body": "# Atlas Lite\n"
}"##;

const SEARCH_RESPONSE: &[u8] = br#"{
  "query": "cafe",
  "total": 1,
  "hits": [{
    "atlasId": "note-1",
    "path": "notes/cafe.md",
    "state": "managed",
    "title": "Caf\u00e9 \ud83d\ude80",
    "revision": "abc123",
    "score": -12.5,
    "snippet": "A <mark>caf\u00e9</mark> \ud83d\ude80"
  }]
}"#;

const VIEW_SUMMARIES: &[u8] = br#"{
  "items": [{
    "id": "view-1",
    "name": "Open work",
    "revision": "view-rev",
    "status": "ok",
    "layout": "table",
    "definition": {"filters": [{"value": {"arbitrary": true}}]}
  }]
}"#;

const VIEW_RESULTS: &[u8] = br#"{
  "view": {
    "id": "view-1",
    "name": "Open work",
    "revision": "view-rev",
    "status": "ok",
    "layout": "table"
  },
  "columns": ["title", "priority"],
  "items": [{
    "id": "note-1",
    "path": "projects/atlas.md",
    "title": "Atlas Lite",
    "state": "managed",
    "revision": "abc123",
    "values": {"priority": {"arbitrary": [1, 2, 3]}},
    "relationValues": {"links": [{"id": "other"}]}
  }],
  "nextCursor": null
}"#;

#[test]
fn parses_representative_note_payloads_without_retaining_frontmatter() {
    let page = parse_note_summary_page(NOTE_PAGE).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id.as_deref(), Some("note-1"));
    assert_eq!(page.items[0].path, "projects/atlas.md");
    assert_eq!(page.items[0].parent_id.as_deref(), Some("projects"));
    assert_eq!(page.items[0].order.as_deref(), Some("a"));
    assert_eq!(page.next_cursor.as_deref(), Some("cursor-2"));

    let document = parse_note_document(NOTE_DOCUMENT).unwrap();
    assert_eq!(document.id.as_deref(), Some("note-1"));
    assert_eq!(document.title, "Atlas Lite");
    assert_eq!(document.body, "# Atlas Lite\n");
}

#[test]
fn parses_search_and_view_payloads_without_retaining_unneeded_values() {
    let search = parse_search_response(SEARCH_RESPONSE).unwrap();
    assert_eq!(search.query, "cafe");
    assert_eq!(search.total, 1);
    assert_eq!(search.hits[0].id.as_deref(), Some("note-1"));
    assert_eq!(search.hits[0].title, "Café 🚀");
    assert_eq!(search.hits[0].snippet, "A <mark>café</mark> 🚀");

    let summaries = parse_view_summaries(VIEW_SUMMARIES).unwrap();
    assert_eq!(summaries.items[0].layout, ViewLayout::Table);
    assert_eq!(summaries.items[0].status, ViewStatus::Ok);

    let results = parse_view_result_page(VIEW_RESULTS).unwrap();
    assert_eq!(results.view.name, "Open work");
    assert_eq!(results.items[0].path, "projects/atlas.md");
    assert_eq!(results.next_cursor, None);
}

#[test]
fn parses_canonical_error_without_retaining_details() {
    let error = parse_api_error(
        br#"{"error":{"code":"ATLAS_INDEX_NOT_READY","message":"Index is not ready","requestId":"req-1","details":{"retryAfter":1}}}"#,
    )
    .unwrap();

    assert_eq!(error.code, "ATLAS_INDEX_NOT_READY");
    assert_eq!(error.message, "Index is not ready");
    assert_eq!(error.request_id, "req-1");
}

#[test]
fn rejects_missing_required_fields_and_unexpected_types() {
    let missing = parse_note_summary_page(
        br#"{"items":[{"id":"note-1","path":"a.md","state":"managed","title":"A","revision":"r"}]}"#,
    );
    assert!(matches!(missing, Err(AtlasDtoError::InvalidJson { .. })));

    let unexpected_type = parse_search_response(br#"{"query":"q","total":"one","hits":[]}"#);
    assert!(matches!(
        unexpected_type,
        Err(AtlasDtoError::InvalidJson { .. })
    ));
}

#[test]
fn rejects_malformed_and_invalid_utf8_json() {
    assert!(matches!(
        parse_view_summaries(br#"{"items":[}"#),
        Err(AtlasDtoError::InvalidJson { .. })
    ));
    assert!(matches!(
        parse_api_error(&[b'{', 0xff, b'}']),
        Err(AtlasDtoError::InvalidJson { .. })
    ));
}

#[test]
fn rejects_oversized_bodies_before_deserialization() {
    let body = vec![b' '; MAX_RESPONSE_BODY_BYTES + 1];
    assert_eq!(
        parse_note_summary_page(&body),
        Err(AtlasDtoError::BodyTooLarge {
            limit: MAX_RESPONSE_BODY_BYTES,
            actual: MAX_RESPONSE_BODY_BYTES + 1,
        })
    );
}
