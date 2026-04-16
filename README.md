# nanomiddleclick

Minimal Rust + Objective-C daemon that emulates MiddleClick-style middle mouse clicks from macOS multitouch gestures.

## Workspace layout

- `nanomiddleclick-core`: gesture/domain logic and config normalization.
- `nanomiddleclick-launchd`: LaunchAgent plist rendering and `launchctl` integration.
- `nanomiddleclick-platform`: macOS integration, Objective-C shim, and safe Rust wrappers.
- `nanomiddleclick`: daemon wiring, runtime state, logging, and CLI entrypoint.

## What it implements

- Physical click rewrite: when the configured finger count is down, left/right click is rewritten to center click.
- Tap-to-click path with the same `fingers`, `allowMoreFingers`, `maxDistanceDelta`, `maxTimeDelta`, `tapToClick`, and `ignoredAppBundles` settings.
- Listener restarts on multitouch device changes, wake, and display reconfiguration.
- `defaults`-driven configuration with `SIGHUP` reload support.
- `launchd` management through `nanomiddleclick daemon on|off`.

## Build

```sh
cargo build --release
```

The macOS platform crate compiles its local Objective-C shim from `nanomiddleclick-platform/shim/` and links against macOS frameworks plus the private `MultitouchSupport.framework`.

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
kill -HUP "$(pgrep -x nanomiddleclick)"
```

## CLI

Run in the foreground:

```sh
target/release/nanomiddleclick
```

Enable verbose logging:

```sh
target/release/nanomiddleclick -v
```

Manage the per-user LaunchAgent:

```sh
target/release/nanomiddleclick daemon on
target/release/nanomiddleclick daemon off
```

`daemon on` writes `~/Library/LaunchAgents/co.myrt.nanomiddleclick.plist`, points it at the current `nanomiddleclick` binary, writes logs to `~/Library/Logs/nanomiddleclick.stdout.log` and `~/Library/Logs/nanomiddleclick.stderr.log`, and loads the agent with `launchctl`.

`daemon off` unloads that LaunchAgent and removes the plist file.

## LaunchAgent template

[launchd/co.myrt.nanomiddleclick.plist](/Users/mishamyrt/Git/mishamyrt/nanomiddleclick/launchd/co.myrt.nanomiddleclick.plist) matches the generated LaunchAgent shape if you want a static reference copy.
