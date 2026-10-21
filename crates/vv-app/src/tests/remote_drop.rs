use super::*;

#[test]
fn smb_locations_keep_their_authority_and_escaping_until_kio_resolves_them() {
    let uri = "smb://user@nas/share/A%20B/%C3%A8%2520%23.mov";
    assert_eq!(parse_uri_list(uri.as_bytes()), [Location::Smb(uri.into())]);
    let resolved = resolve_with(parse_uri_list(uri.as_bytes()), |received| {
        assert_eq!(received, uri);
        Ok("/run/user/1000/kio-fuse-123/smb/nas/share/A B/è%20#.mov".into())
    });
    assert_eq!(
        resolved.paths,
        [PathBuf::from(
            "/run/user/1000/kio-fuse-123/smb/nas/share/A B/è%20#.mov"
        )]
    );
    assert!(resolved.failures.is_empty());
}

#[test]
fn mixed_local_and_remote_drops_keep_their_order() {
    let locations = parse_uri_list(b"# comment\r\nfile:///mnt/nas/local%20file.mp4\r\nsmb://user@nas/share/remote.mp4\r\nfile://localhost/tmp/end.wav\r\n");
    let resolved = resolve_with(locations, |_| {
        Ok("/run/user/1000/kio-fuse-123/remote.mp4".into())
    });
    assert_eq!(
        resolved.paths,
        [
            PathBuf::from("/mnt/nas/local file.mp4"),
            PathBuf::from("/run/user/1000/kio-fuse-123/remote.mp4"),
            PathBuf::from("/tmp/end.wav"),
        ]
    );
}

#[test]
fn local_drops_do_not_need_kio_fuse() {
    let resolved = resolve_with(parse_uri_list(b"file:///mnt/cifs/clip.mp4"), |_| {
        panic!("local path must not call KIO")
    });
    assert_eq!(resolved.paths, [PathBuf::from("/mnt/cifs/clip.mp4")]);
}

#[test]
fn a_failed_remote_file_does_not_discard_the_other_files_or_expose_credentials() {
    let locations = parse_uri_list(b"smb://user:secret@nas/share/failed%20clip.mp4\nfile:///tmp/local.mp4\nsmb://nas/share/good.mp4");
    let resolved = resolve_with(locations, |uri| {
        if uri.ends_with("good.mp4") {
            Ok("/run/user/1000/kio-fuse-123/good.mp4".into())
        } else {
            Err(ResolveError::MountFailed)
        }
    });
    assert_eq!(
        resolved.paths,
        [
            PathBuf::from("/tmp/local.mp4"),
            PathBuf::from("/run/user/1000/kio-fuse-123/good.mp4")
        ]
    );
    assert_eq!(resolved.failures.len(), 1);
    assert_eq!(resolved.failures[0].name, "failed clip.mp4");
    assert_eq!(resolved.failures[0].error, ResolveError::MountFailed);
}

#[test]
fn local_file_authorities_are_not_confused_with_remote_hosts() {
    assert_eq!(
        parse_uri_list(b"file://nas/share/video.mp4\nfile://LOCALHOST/tmp/clip.mp4"),
        [Location::Local(PathBuf::from("/tmp/clip.mp4"))]
    );
}

#[test]
fn a_location_without_a_path_never_uses_its_credentials_as_the_error_label() {
    let resolved = resolve_with(parse_uri_list(b"smb://user:secret@nas"), |_| {
        Err(ResolveError::MountFailed)
    });
    assert_eq!(resolved.failures[0].name, "SMB");
}

#[test]
fn unsupported_and_malformed_locations_are_ignored() {
    assert!(parse_uri_list(b"https://example.com/video.mp4\nfile://\nfile:///tmp/%00.mp4\nsmb://host/\0video.mp4\n").is_empty());
}

#[test]
fn native_paths_preserve_non_utf8_filenames() {
    assert_eq!(
        parse_uri_list(b"file:///mnt/share/%FF.mp4"),
        [Location::Local(PathBuf::from(OsString::from_vec(
            b"/mnt/share/\xff.mp4".to_vec()
        )))]
    );
}

#[test]
fn literal_mount_replies_preserve_spaces_newlines_and_reserved_characters() {
    for name in [
        "A B/è%20#.mp4",
        "trailing space ",
        "embedded\nnewline",
        "trailing newline\n",
    ] {
        let path = format!("/run/user/1000/kio-fuse-123/{name}");
        assert_eq!(
            parse_mount_reply(format!("   {path}").into_bytes()).unwrap(),
            PathBuf::from(path)
        );
    }
}

#[test]
fn empty_relative_or_nul_mount_replies_are_rejected() {
    for reply in [
        b"".as_slice(),
        b"   ",
        b"   relative.mp4",
        b"   /tmp/a\0.mp4",
    ] {
        assert_eq!(
            parse_mount_reply(reply.to_vec()),
            Err(ResolveError::InvalidReply)
        );
    }
}
