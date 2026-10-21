use super::*;

#[test]
#[cfg(target_os = "linux")]
fn case_insensitive_matches_every_spelling() {
    assert_eq!(
        case_insensitive(&["mp4", "tiff"]),
        ["[mM][pP]4", "[tT][iI][fF][fF]"]
    );
}

#[test]
#[cfg(not(target_os = "linux"))]
fn case_insensitive_keeps_plain_extensions() {
    assert_eq!(case_insensitive(&["mp4", "tiff"]), ["mp4", "tiff"]);
}

#[cfg(target_os = "linux")]
fn queue_resolved_drop(
    app: &mut VenturiApp,
    target: DropTarget,
    resolved: crate::remote_drop::ResolvedDrop,
) {
    let (tx, rx) = mpsc::channel();
    tx.send(resolved).unwrap();
    app.pending_remote_drops.push(PendingRemoteDrop {
        target,
        epoch: app.session.epoch(),
        rx,
    });
}

#[test]
#[cfg(target_os = "linux")]
fn remote_drop_keeps_its_original_project_folder_and_reports_partial_failures() {
    let mut app = VenturiApp::default();
    let folder = app.session.project.folders.insert(vv_core::MediaFolder {
        name: "Original destination".into(),
        parent: None,
    });
    let other = app.session.project.folders.insert(vv_core::MediaFolder {
        name: "Current hover".into(),
        parent: None,
    });
    let directory = std::env::temp_dir().join("vv-remote-drop-test");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("remote clip.wav");
    vv_media::test_support::ffmpeg(
        &["-f", "lavfi", "-i", "sine=frequency=440:duration=0.1"],
        &path,
    );
    queue_resolved_drop(
        &mut app,
        DropTarget::Project(Some(folder)),
        crate::remote_drop::ResolvedDrop {
            paths: vec![path.clone()],
            failures: vec![crate::remote_drop::Failure {
                name: "unavailable.mp4".into(),
                error: crate::remote_drop::ResolveError::MountFailed,
            }],
        },
    );
    app.media_pool_state.file_drop_folder = Some(other);
    app.library_ui.tab = library_ui::PoolTab::Library;
    let ctx = egui::Context::default();
    app.poll_remote_drops(&ctx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.session.has_running_jobs() {
        app.poll_session(&ctx);
        assert!(
            std::time::Instant::now() < deadline,
            "import did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let item = app
        .session
        .project
        .media_pool
        .values()
        .find(|item| item.path == path)
        .unwrap();
    assert_eq!(item.folder, Some(folder));
    assert!(
        app.project_error
            .as_ref()
            .unwrap()
            .contains("unavailable.mp4")
    );
    assert!(app.pending_remote_drops.is_empty());
}

#[test]
#[cfg(target_os = "linux")]
fn remote_drop_from_a_replaced_project_is_discarded() {
    let mut app = VenturiApp::default();
    queue_resolved_drop(
        &mut app,
        DropTarget::Project(None),
        crate::remote_drop::ResolvedDrop {
            paths: vec!["/tmp/previous-project.mp4".into()],
            failures: vec![crate::remote_drop::Failure {
                name: "old-error.mp4".into(),
                error: crate::remote_drop::ResolveError::MountFailed,
            }],
        },
    );
    app.session.new_project();
    app.poll_remote_drops(&egui::Context::default());
    assert!(!app.session.has_running_jobs());
    assert!(app.session.project.media_pool.is_empty());
    assert!(app.project_error.is_none());
    assert!(app.pending_remote_drops.is_empty());
}
