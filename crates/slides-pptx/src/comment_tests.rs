//! Tests for comment persistence in the 900Slides custom XML manifest
//! (Wave 17, component 2).

use std::io::Read;

use slides_core::{Comment, CommentAnchor, CommentThread};

use crate::{load, save};

const SLIDE_ID: &str = "ppt/slides/slide1.xml";

/// Loads a blank PPTX into a session and attaches a representative comment
/// thread (root + reply, assigned, resolved) anchored to the single slide.
fn session_with_thread() -> crate::session::Session {
    let mut session = load(crate::create_blank_pptx().as_slice()).expect("load should succeed");
    session.deck_mut().comments = vec![CommentThread {
        id: "thread1".to_string(),
        anchor: CommentAnchor::Slide {
            slide_id: SLIDE_ID.to_string(),
        },
        comments: vec![
            Comment {
                id: "c1".to_string(),
                author: "Ada Lovelace".to_string(),
                body: "Nice work on this slide".to_string(),
                timestamp: "2026-07-29T12:00:00Z".to_string(),
                resolved: false,
            },
            Comment {
                id: "c2".to_string(),
                author: "Alan Turing".to_string(),
                body: "Agreed, ship it".to_string(),
                timestamp: "2026-07-29T12:05:00Z".to_string(),
                resolved: false,
            },
        ],
        assigned_to: Some("Grace Hopper".to_string()),
        resolved: true,
    }];
    session
}

/// Extracts the manifest part (`customXml/item1.xml`) text from a package.
fn manifest_xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let mut file = archive.by_name("customXml/item1.xml").unwrap();
    let mut xml = String::new();
    file.read_to_string(&mut xml).unwrap();
    xml
}

#[test]
fn save_comments_in_manifest() {
    let session = session_with_thread();
    let bytes = save(&session).expect("save should succeed");

    let xml = manifest_xml(&bytes);
    assert!(
        xml.contains("<comments>"),
        "manifest should contain a <comments> section: {xml}"
    );
    // Author and body text must survive into the manifest payload.
    assert!(xml.contains("Ada Lovelace"), "missing author in manifest");
    assert!(
        xml.contains("Nice work on this slide"),
        "missing body in manifest"
    );
    assert!(xml.contains("Grace Hopper"), "missing assignee in manifest");
}

#[test]
fn load_comments_from_manifest() {
    let session = session_with_thread();
    let bytes = save(&session).expect("save should succeed");

    let reloaded = load(&bytes).expect("reload should succeed");
    let comments = &reloaded.deck().comments;
    assert_eq!(comments.len(), 1, "exactly one thread should reload");

    let thread = &comments[0];
    assert_eq!(thread.id, "thread1");
    assert_eq!(thread.assigned_to.as_deref(), Some("Grace Hopper"));
    assert!(thread.resolved, "thread resolved flag should round-trip");

    match &thread.anchor {
        CommentAnchor::Slide { slide_id } => {
            assert_eq!(slide_id, SLIDE_ID);
        }
        other => panic!("expected Slide anchor, got {other:?}"),
    }

    assert_eq!(thread.comments.len(), 2, "root + reply should reload");
    assert_eq!(thread.comments[0].author, "Ada Lovelace");
    assert_eq!(thread.comments[0].body, "Nice work on this slide");
    assert_eq!(thread.comments[1].author, "Alan Turing");
}

#[test]
fn round_trip_comments_preserves_thread() {
    let bytes = save(&session_with_thread()).expect("first save should succeed");
    let first = load(&bytes).expect("first reload should succeed");

    // Save again with no edits, then reload: comments must be structurally
    // identical across the double round-trip.
    let bytes_again = save(&first).expect("second save should succeed");
    let second = load(&bytes_again).expect("second reload should succeed");

    assert_eq!(first.deck().comments, second.deck().comments);
    assert_eq!(second.deck().comments.len(), 1);
    assert_eq!(second.deck().comments[0].id, "thread1");
    assert_eq!(second.deck().comments[0].comments.len(), 2);
}

#[test]
fn no_manifest_comments_section_loads_empty() {
    // A freshly created blank deck has no comments.
    let blank = crate::create_blank_pptx();
    let session = load(blank.as_slice()).expect("blank should load");
    assert!(
        session.deck().comments.is_empty(),
        "blank deck should start with no comments"
    );

    let bytes = save(&session).expect("save should succeed");

    // The manifest must not carry a comments section for a comment-free deck.
    let xml = manifest_xml(&bytes);
    assert!(
        !xml.contains("<comments"),
        "comment-free deck should have no <comments> section: {xml}"
    );

    // And reloading yields an empty comment list.
    let reloaded = load(&bytes).expect("reload should succeed");
    assert!(
        reloaded.deck().comments.is_empty(),
        "reloaded deck should have no comments"
    );
}

#[test]
fn comment_free_manifest_is_self_closing_and_deterministic() {
    // A freshly created blank deck has no comments.
    let blank = crate::create_blank_pptx();
    let session = load(blank.as_slice()).expect("blank should load");
    assert!(
        session.deck().comments.is_empty(),
        "blank deck should start with no comments"
    );

    let bytes = save(&session).expect("save should succeed");

    // The manifest must carry no comments section for a comment-free deck: the
    // root element stays self-closing, exactly as before this change, so the
    // byte-for-byte guarantee for untouched (regenerated) parts is preserved.
    let xml = manifest_xml(&bytes);
    assert!(
        !xml.contains("comments"),
        "comment-free deck should have no comments markup: {xml}"
    );
    assert!(
        xml.contains("/>"),
        "comment-free manifest root should be self-closing: {xml}"
    );

    // Reloading yields an empty comment list.
    let reloaded = load(&bytes).expect("reload should succeed");
    assert!(
        reloaded.deck().comments.is_empty(),
        "reloaded deck should have no comments"
    );

    // Saving the same session twice must be byte-stable, i.e. the regenerated
    // manifest is deterministic (no nondeterministic comment serialization).
    let bytes_a = save(&session).expect("save A should succeed");
    let bytes_b = save(&session).expect("save B should succeed");
    assert_eq!(
        bytes_a, bytes_b,
        "no-op double save must be byte-for-byte identical"
    );
}

#[test]
fn text_range_anchor_round_trips() {
    // Exercise a non-Slide anchor variant to confirm the serde tag survives.
    let mut session = load(crate::create_blank_pptx().as_slice()).expect("load should succeed");
    session.deck_mut().comments = vec![CommentThread {
        id: "tr1".to_string(),
        anchor: CommentAnchor::TextRange {
            slide_id: SLIDE_ID.to_string(),
            shape_id: "2".to_string(),
            start: 3,
            end: 9,
        },
        comments: vec![Comment {
            id: "c1".to_string(),
            author: "Editor".to_string(),
            body: "Fix this phrase".to_string(),
            timestamp: "2026-07-29T13:00:00Z".to_string(),
            resolved: false,
        }],
        assigned_to: None,
        resolved: false,
    }];

    let bytes = save(&session).expect("save should succeed");
    let reloaded = load(&bytes).expect("reload should succeed");
    assert_eq!(reloaded.deck().comments, session.deck().comments);

    match &reloaded.deck().comments[0].anchor {
        CommentAnchor::TextRange {
            slide_id,
            shape_id,
            start,
            end,
        } => {
            assert_eq!(slide_id, SLIDE_ID);
            assert_eq!(shape_id, "2");
            assert_eq!(*start, 3);
            assert_eq!(*end, 9);
        }
        other => panic!("expected TextRange anchor, got {other:?}"),
    }
}
