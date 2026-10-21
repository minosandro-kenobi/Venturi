# Network media on Linux

Files on a mounted SMB/CIFS or NFS filesystem can be imported using their
ordinary paths, such as `/mnt/share/video.mp4`.

## Dragging SMB files from Dolphin on Wayland

Dolphin can supply an `smb://user@server/share/video.mp4` URI when a file is
dragged from its network browser. Venturi resolves this URI through KDE's
[KIO-FUSE service](https://invent.kde.org/system/kio-fuse) before passing a
filesystem path to FFmpeg. Authentication and network access are handled by
KDE; the file must be accessible in Dolphin.

The desktop needs `kio-fuse` and the `dbus-send` command. On Debian/Ubuntu:

```sh
sudo apt install kio-fuse dbus-bin
```

These are optional runtime requirements for SMB drops, with no additional
Rust dependencies. Local files and already mounted network paths do not use
the resolver.

Resolution runs in the background and retains the original project/library
folder selected for the drop. A failure shows an error; other successful
files in the same drop are still imported. Results belonging to a project
that has since been replaced are discarded.

Venturi does not first copy the whole video into a local project folder.
KIO-FUSE manages its own caching. The project stores the resolved filesystem
path: if KDE recreates that mount under a different path after logout, use
**Relink media** to reconnect it. A persistent filesystem mount avoids this
path change.

This integration handles Dolphin's SMB URIs on Wayland. X11 drag-and-drop is
handled by winit, which does not pass remote SMB URIs to Venturi; use a
mounted filesystem path there.
