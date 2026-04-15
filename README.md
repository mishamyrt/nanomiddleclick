# nanomiddleclick

Minimal Rust + Objective-C daemon that emulates MiddleClick-style middle mouse clicks from macOS multitouch gestures.

## What it implements

- Physical click rewrite: when the configured finger count is down, left/right click is rewritten to center click.
- Tap-to-click path with the same `fingers`, `allowMoreFingers`, `maxDistanceDelta`, `maxTimeDelta`, `tapToClick`, and `ignoredAppBundles` settings.
- Listener restarts on multitouch device changes, wake, and display reconfiguration.
- `defaults`-driven configuration with `SIGHUP` reload support.
- `launchd` template for a per-user `LaunchAgent`.

## Build

```sh
cargo build --release
```

The build uses a local Objective-C shim compiled by `build.rs`, and links against macOS frameworks plus the private `MultitouchSupport.framework`.

## Defaults domain

The daemon reads settings from:

```sh
co.myrt.nanomiddleclick
```

Supported keys:

```sh
defaults write co.myrt.nanomiddleclick fingers -int 4
defaults write co.myrt.nanomiddleclick allowMoreFingers -bool true
defaults write co.myrt.nanomiddleclick maxDistanceDelta -float 0.03
defaults write co.myrt.nanomiddleclick maxTimeDelta -int 150
defaults write co.myrt.nanomiddleclick tapToClick -bool true
defaults write co.myrt.nanomiddleclick ignoredAppBundles -array com.apple.finder com.apple.Terminal
```

Reload the running daemon after changing defaults:

```sh
kill -HUP "$(pgrep -x nanomiddleclickd)"
```

## LaunchAgent

Use [launchd/co.myrt.nanomiddleclickd.plist](/Users/mishamyrt/Git/mishamyrt/nanomiddleclick/launchd/co.myrt.nanomiddleclickd.plist) as a template.

Replace:

- `__EXECUTABLE__` with the absolute path to `target/release/nanomiddleclickd`
- `__STDOUT_PATH__` with a writable log path
- `__STDERR_PATH__` with a writable log path

Then load it:

```sh
launchctl unload ~/Library/LaunchAgents/co.myrt.nanomiddleclickd.plist 2>/dev/null || true
cp launchd/co.myrt.nanomiddleclickd.plist ~/Library/LaunchAgents/co.myrt.nanomiddleclickd.plist
launchctl load ~/Library/LaunchAgents/co.myrt.nanomiddleclickd.plist
```
