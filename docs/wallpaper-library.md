# Wallpaper Library

The library stores its sources, download root, and provider records in
`$XDG_CONFIG_HOME/matugen-studio/wallpaper-library.json`. On first launch it
imports existing `wallpaperFolder_*` selections and Wallhaven history from
localStorage. The old keys remain for rollback. Library sources are global;
the selected KDE monitor and color monitor remain separate controls.

The default permanent download directory is
`<XDG_PICTURES_DIR>/Wallpapers/Matugen Studio`. If the XDG pictures directory
is unavailable, the app uses `<XDG_DATA_HOME>/Wallpapers/Matugen Studio`
(or `~/.local/share/Wallpapers/Matugen Studio` if XDG data is unavailable).
Each remote provider has a subdirectory. Changing the download root affects
future saves and keeps the previous root as a library source. Removing a user
source removes only the source registration, never its files.
Automatic discovery considers `<XDG_PICTURES_DIR>/Wallpapers` when it exists,
never the entire XDG pictures directory. Other folders under Pictures require
an explicit user action.
Automatic discovery runs when the library is first initialized. Add a folder
explicitly if it is created later; rescanning refreshes registered sources but
does not silently register new folders.

Wallhaven **Load Colors** uses a saved download when available, or otherwise
fetches to `$XDG_CACHE_HOME/matugen-studio/wallhaven`. **Apply and Generate**
uses the saved path when available; otherwise it promotes the cached image to
`$XDG_DATA_HOME/matugen-studio/active-wallpapers/wallhaven` before applying
it to KDE. This managed active copy survives cache cleanup and is not a saved
download. **Download** copies a valid cached or managed active image when
available, or fetches it directly, and saves it under `DownloadRoot/Wallhaven`. A saved
wallpaper appears in the Local Downloads filter immediately. The library
checks cached and saved paths again on startup; a missing file loses that
status. Clearing cache does not delete permanent downloads.

Presets still store a wallpaper path snapshot. New actions use a permanent
path when the wallpaper is saved. A preset previously created from a
cache-only wallpaper may still refer to that cache path until it is saved or
exported with its embedded wallpaper. Applying such a preset while the cache
file still exists promotes its KDE wallpaper path to a managed copy; the
stored preset itself is left untouched.

A KDE wallpaper applied by an older version directly from cache needs one
explicit reapply to switch its KDE path to the managed copy. The app does not
silently rewrite existing KDE monitor assignments at startup.
