//! File-manager locations converted to paths readable by the media pipeline.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, PartialEq)]
pub(crate) enum Location {
    Local(PathBuf),
    Smb(String),
}

pub(crate) fn smb_uri(path: &std::path::Path) -> Option<&str> {
    path.to_str().filter(|uri| {
        uri.get(..6)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("smb://"))
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ResolveError {
    MissingHelper,
    MissingKioFuse,
    MountFailed,
    InvalidReply,
}

pub(crate) struct Failure {
    pub name: String,
    pub error: ResolveError,
}

#[derive(Default)]
pub(crate) struct ResolvedDrop {
    pub paths: Vec<PathBuf>,
    pub failures: Vec<Failure>,
}

pub(crate) fn parse_uri_list(bytes: &[u8]) -> Vec<Location> {
    bytes
        .split(|&b| b == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .filter(|line| !line.starts_with(b"#") && !line.contains(&0))
        .filter_map(|line| {
            if line
                .get(..6)
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case(b"smb://"))
            {
                return std::str::from_utf8(line)
                    .ok()
                    .map(|uri| Location::Smb(uri.to_owned()));
            }
            let rest = line.strip_prefix(b"file://")?;
            let slash = rest.iter().position(|&b| b == b'/')?;
            let host = &rest[..slash];
            if !host.is_empty() && !host.eq_ignore_ascii_case(b"localhost") {
                return None;
            }
            let path = percent_decode(&rest[slash..]);
            (!path.contains(&0)).then(|| Location::Local(PathBuf::from(OsString::from_vec(path))))
        })
        .collect()
}

pub(crate) fn resolve_locations(locations: Vec<Location>) -> ResolvedDrop {
    resolve_with(locations, mount_smb)
}

fn resolve_with(
    locations: Vec<Location>,
    mut mount: impl FnMut(&str) -> Result<PathBuf, ResolveError>,
) -> ResolvedDrop {
    let mut result = ResolvedDrop::default();
    for location in locations {
        match location {
            Location::Local(path) => result.paths.push(path),
            Location::Smb(uri) => match mount(&uri) {
                Ok(path) => result.paths.push(path),
                Err(error) => {
                    let name = uri
                        .split_once("://")
                        .and_then(|(_, rest)| rest.split_once('/'))
                        .map(|(_, path)| path.trim_end_matches('/'))
                        .and_then(|path| path.rsplit('/').next())
                        .filter(|name| !name.is_empty())
                        .unwrap_or("SMB");
                    result.failures.push(Failure {
                        name: String::from_utf8_lossy(&percent_decode(name.as_bytes()))
                            .into_owned(),
                        error,
                    });
                }
            },
        }
    }
    result
}

fn mount_smb(uri: &str) -> Result<PathBuf, ResolveError> {
    let output = Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply=literal",
            "--reply-timeout=30000",
            "--dest=org.kde.KIOFuse",
            "/org/kde/KIOFuse",
            "org.kde.KIOFuse.VFS.mountUrl",
        ])
        .arg(format!("string:{uri}"))
        .output()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => ResolveError::MissingHelper,
            _ => ResolveError::MountFailed,
        })?;
    if !output.status.success() {
        return Err(
            if String::from_utf8_lossy(&output.stderr)
                .contains("org.freedesktop.DBus.Error.ServiceUnknown")
            {
                ResolveError::MissingKioFuse
            } else {
                ResolveError::MountFailed
            },
        );
    }
    parse_mount_reply(output.stdout)
}

fn parse_mount_reply(bytes: Vec<u8>) -> Result<PathBuf, ResolveError> {
    // dbus-send indents even literal strings; it adds no trailing newline.
    let path = bytes.strip_prefix(b"   ").unwrap_or(&bytes);
    if !path.starts_with(b"/") || path.contains(&0) {
        return Err(ResolveError::InvalidReply);
    }
    Ok(PathBuf::from(OsString::from_vec(path.to_vec())))
}

fn percent_decode(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "tests/remote_drop.rs"]
mod tests;
