//! End-to-End Keyboard Workflow Integration Test (P6-P7 / T13).
//!
//! Validates the entire cohesive workflow crossing all subsystems:
//! `vault -> open note -> vim edit -> flash jump -> wrap color -> create link -> resolve backlinks -> rename file with link cascade -> atomic save -> restart session`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use truff_core::markdown::facts::MarkdownFacts;
use truff_core::markdown::style::apply_text_color;
use truff_core::text::Buffer;
use truff_core::vim::Key;
use truff_store::VaultPaths;
use truff_store::files::{atomic_write_file, compute_file_fingerprint, compute_fingerprint};
use truff_store::index::{FileId, IndexDatabase, Resolution, Target};
use truff_store::operations::{FileOpPlan, LinkSelection};
use truff_store::recovery::RecoveryManager;
use truff_store::session::{
    BufferRestoreState, SessionBufferEntry, SessionData, SessionViewEntry, restore_session,
    save_session,
};
use truff_ui::overlays::FlashOverlay;
use truff_ui::shell::Shell;

fn temp_test_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("truff_e2e_{name}_{}_{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

#[test]
fn test_end_to_end_keyboard_workflow() {
    // -----------------------------------------------------------------------
    // Step 1: Vault initialization
    // -----------------------------------------------------------------------
    let temp_dir = temp_test_dir("full_workflow");
    let vault = VaultPaths::new(temp_dir.clone());
    let mut db = IndexDatabase::open_in_memory().expect("failed to open index db");
    let recovery = RecoveryManager::new(temp_dir.clone(), 10 * 1024 * 1024);

    let target_file_rel = PathBuf::from("target_note.md");
    let target_file_abs = temp_dir.join(&target_file_rel);
    let target_initial_content = "# Target Note\nThis is the initial target note content.\n";
    fs::write(&target_file_abs, target_initial_content).expect("failed to write target note");

    let target_facts =
        MarkdownFacts::extract(target_initial_content).expect("extract target facts");
    db.index_file(
        FileId(1),
        "target_note.md",
        "Target Note",
        "fp_target_1",
        1,
        &target_facts,
        target_initial_content,
    )
    .expect("index target note");

    // -----------------------------------------------------------------------
    // Step 2: Open note in Shell
    // -----------------------------------------------------------------------
    let main_file_rel = PathBuf::from("main_note.md");
    let main_file_abs = temp_dir.join(&main_file_rel);
    let main_initial_content = "# Main Note\nWelcome to Truff editor.\nSome interesting text here.\nSee also other notes.\n";
    fs::write(&main_file_abs, main_initial_content).expect("failed to write main note");

    let mut shell = Shell::new();
    shell.set_editor_text(main_initial_content);
    assert_eq!(shell.editor_text(), main_initial_content);

    // -----------------------------------------------------------------------
    // Step 3: Vim edit (Normal -> Insert -> Keystrokes -> Normal)
    // -----------------------------------------------------------------------
    shell.feed_key(Key::Char('i'));
    for ch in "Prefix: ".chars() {
        shell.feed_key(Key::Char(ch));
    }
    shell.feed_key(Key::Esc);
    assert!(shell.editor_text().starts_with("Prefix: # Main Note"));

    // -----------------------------------------------------------------------
    // Step 4: Flash jump (navigate to word 'interesting')
    // -----------------------------------------------------------------------
    let mut flash = FlashOverlay::new();
    let visible_text = shell.editor_text();
    flash.activate(&visible_text, 0);
    assert!(flash.is_active(), "Flash overlay must be active");

    let target_candidate = flash
        .targets()
        .iter()
        .find(|t| t.text == "interesting")
        .expect("must find flash target for 'interesting'");
    let target_label = target_candidate.label.clone();
    let target_range = target_candidate.source_range.clone();

    let mut jump_range = None;
    for ch in target_label.chars() {
        jump_range = flash.feed_char(ch);
    }
    assert_eq!(
        jump_range,
        Some(target_range.clone()),
        "Flash jump must land on exact target range"
    );
    assert!(!flash.is_active(), "Flash must deactivate after jump");

    // -----------------------------------------------------------------------
    // Step 5: Wrap color (apply text color wrapper to 'interesting')
    // -----------------------------------------------------------------------
    let current_text = shell.editor_text();
    let color_tx =
        apply_text_color(&current_text, target_range, "#3388ff").expect("apply color wrapper");
    let mut buffer = Buffer::from_str(&current_text);
    buffer.apply(color_tx).expect("apply color transaction");
    let text_after_color = buffer.as_str();
    assert!(
        text_after_color.contains("<span style=\"color:#3388ff\">interesting</span>"),
        "Buffer text must contain wrapped color span"
    );
    shell.set_editor_text(&text_after_color);

    // -----------------------------------------------------------------------
    // Step 6: Create link (convert 'other notes' into [[target_note|notes ref]])
    // -----------------------------------------------------------------------
    let current_text = shell.editor_text();
    let link_target_str = "other notes";
    let link_start = current_text
        .find(link_target_str)
        .expect("find phrase to link");
    let link_range = link_start..(link_start + link_target_str.len());
    let link_tx = LinkSelection::wikilink(link_range, "target_note", Some("notes ref"));
    let mut buffer = Buffer::from_str(&current_text);
    buffer.apply(link_tx).expect("apply wikilink transaction");
    let text_after_link = buffer.as_str();
    assert!(
        text_after_link.contains("[[target_note|notes ref]]"),
        "Buffer text must contain created wikilink"
    );
    shell.set_editor_text(&text_after_link);

    // -----------------------------------------------------------------------
    // Step 7: Resolve backlinks via IndexDatabase
    // -----------------------------------------------------------------------
    let main_facts = MarkdownFacts::extract(&text_after_link).expect("extract main facts");
    db.index_file(
        FileId(2),
        "main_note.md",
        "Main Note",
        "fp_main_1",
        1,
        &main_facts,
        &text_after_link,
    )
    .expect("index main note");

    // Resolve target_note
    let target = Target::parse("target_note");
    let resolution = db.resolver().resolve(FileId(2), &target);
    assert_eq!(
        resolution,
        Resolution::Found(FileId(1), None),
        "Resolver must return Found(FileId(1), None) for target_note"
    );

    // Query backlinks for target_note (FileId 1)
    let backlinks = db
        .get_backlinks(FileId(1), None, 0, 10)
        .expect("query backlinks");
    assert_eq!(backlinks.total_matches, 1, "Must have 1 backlink");
    assert_eq!(
        backlinks.occurrences[0].source_path, "main_note.md",
        "Backlink must point from main_note.md"
    );

    // Save main_note before rename so that no dirty buffer is blocked
    fs::write(&main_file_abs, &text_after_link).expect("write main note to disk");

    // -----------------------------------------------------------------------
    // Step 8: Rename file with link cascade
    // -----------------------------------------------------------------------
    let mut plan = FileOpPlan::rename(&vault, "target_note.md", "target_note_v2.md")
        .expect("plan rename operation");
    let dirty_buffers: Vec<PathBuf> = Vec::new();
    plan.validate(&vault, &dirty_buffers, &db)
        .expect("validate rename plan");
    plan.stage(&vault, db.connection_mut())
        .expect("stage rename plan");
    plan.apply(&vault, db.connection_mut())
        .expect("apply rename plan");

    // target_note.md was renamed to target_note_v2.md
    assert!(!target_file_abs.exists(), "Old target path must be removed");
    let new_target_abs = temp_dir.join("target_note_v2.md");
    assert!(new_target_abs.exists(), "New target path must exist");

    // main_note.md on disk was rewritten with cascaded link update
    let rewritten_main = fs::read_to_string(&main_file_abs).expect("read rewritten main note");
    assert!(
        rewritten_main.contains("[[target_note_v2|notes ref]]"),
        "Cascaded link rewrite must update link target to target_note_v2"
    );
    shell.set_editor_text(&rewritten_main);

    // -----------------------------------------------------------------------
    // Step 9: Atomic save
    // -----------------------------------------------------------------------
    let final_editor_text = shell.editor_text();
    let expected_fp = compute_fingerprint(final_editor_text.as_bytes());
    atomic_write_file(&main_file_abs, &final_editor_text).expect("atomic write main note");
    let actual_fp =
        compute_file_fingerprint(&main_file_abs).expect("compute fingerprint from disk");
    assert_eq!(
        expected_fp, actual_fp,
        "Atomic save fingerprint must match editor content"
    );

    // -----------------------------------------------------------------------
    // Step 10: Restart session
    // -----------------------------------------------------------------------
    let session = SessionData {
        version: 1,
        vault_path: temp_dir.clone(),
        active_buffer: Some(main_file_rel.clone()),
        buffers: vec![
            SessionBufferEntry {
                path: main_file_rel.clone(),
                is_dirty: false,
                last_saved_fingerprint: Some(actual_fp),
            },
            SessionBufferEntry {
                path: PathBuf::from("target_note_v2.md"),
                is_dirty: false,
                last_saved_fingerprint: None,
            },
        ],
        views: vec![SessionViewEntry {
            buffer_path: main_file_rel.clone(),
            cursor_anchor: 10,
            cursor_head: 10,
            top_line: 0,
        }],
        active_view_index: Some(0),
        jump_history: vec![(0, 10)],
    };

    save_session(&vault, &session).expect("save session state");

    // Drop in-memory objects to simulate full application shutdown
    drop(shell);
    drop(db);

    // Restore session on simulated restart
    let restored = restore_session(&vault, &recovery).expect("restore session on restart");
    assert_eq!(
        restored.active_buffer_path,
        Some(main_file_rel),
        "Active buffer path must match"
    );
    assert_eq!(restored.buffers.len(), 2, "Session buffers count must be 2");

    let active_entry = &restored.buffers[0];
    if let BufferRestoreState::Loaded(ref content) = active_entry.state {
        assert_eq!(content, &final_editor_text);
        assert!(content.contains("[[target_note_v2|notes ref]]"));
        assert!(content.contains("<span style=\"color:#3388ff\">interesting</span>"));
    } else {
        panic!("Active buffer must be loaded immediately on session restore");
    }

    // Inactive buffer is left lazy
    let inactive_entry = &restored.buffers[1];
    assert!(
        matches!(inactive_entry.state, BufferRestoreState::Lazy),
        "Inactive buffer must be lazy loaded"
    );
}
