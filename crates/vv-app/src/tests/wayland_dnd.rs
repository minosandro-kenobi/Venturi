use super::*;
use crate::remote_drop::Location;

#[test]
fn uri_list_yields_decoded_local_paths() {
    let list = b"# comment\r\nfile:///home/me/a%20b.mp4\r\nfile://localhost/tmp/%C3%A8.wav\r\nhttps://x/y\r\n";
    assert_eq!(
        parse_uri_list(list),
        vec![
            Location::Local(PathBuf::from("/home/me/a b.mp4")),
            Location::Local(PathBuf::from("/tmp/è.wav"))
        ]
    );
}
