<p align="center">
    <img src="./docs/logo.svg" width="50px" />
</p>

<h1 align="center">nanomiddleclick</h1>

<p align="center">
  <a href="https://github.com/mishamyrt/nanomiddleclick/actions/workflows/qa.yml">
      <img src="https://github.com/mishamyrt/nanomiddleclick/actions/workflows/qa.yml/badge.svg" />
  </a>
</p>

Lightweight daemon for middle-click emulation on macOS.

## Features

- Supports:
  - MacBook onboard trackpad;
  - Magic Trackpad;
  - Magic Mouse.
- Uses 12 MB of RAM (compared to 40+ for [MiddleClick](https://github.com/artginzburg/MiddleClick))
- Highly configurable.

## What for?

To open links in a new tab or close tabs in browsers with a single action using one hand (without pressing cmd) and paste highlighted text in the Terminal app.

## Installation

### From source

To build the project, you must have the Rust toolchain installed on your computer.

```sh
make
make install
```

## Setup

Once the app is installed, you need to start the background process. To do this, run the following in your terminal:

```
nanomiddleclick daemon on
```

## Configuration

The daemon reads settings from `co.myrt.nanomiddleclick`. After changing the configuration, you must restart the daemon:

```sh
kill -HUP "$(pgrep -x nanomiddleclick)"
```

### Number of Fingers

You can use any number of fingers (up to 10) to middle-click.

> ☝️ Note: setting fingers to 2 will conflict with normal two-finger right-clicks.

```sh
defaults write co.myrt.nanomiddleclick fingers -int 4
```

### Allow to click with more than the defined number of fingers.

This is useful if your second hand accidentally touches the touchpad.

Disabled by default.

```sh
defaults write co.myrt.nanomiddleclick allowMoreFingers -bool true
```

### Tapping tuning

The default values for these settings should work for most users, but if they aren't working correctly for you, you should start by adjusting them.

#### Max Distance Delta

The maximum distance the cursor can travel between touch and release for a tap to be considered valid. The position is normalized and values go from 0 to 1.

Default is 0.05.

```sh
defaults write co.myrt.nanomiddleclick maxDistanceDelta -float 0.03
```

#### Max Time Delta

The maximum interval in milliseconds between touch and release for a tap to be considered valid.

Default is 300

```sh
defaults write co.myrt.nanomiddleclick maxTimeDelta -int 150
```

### Ignored apps

Some (actually very rare) applications have built-in separate support for 3-finger taps. To avoid conflicts with them and prevent the daemon from running, use can use `ignoredAppBundles` parameter.

Default is empty.

```sh
defaults write co.myrt.nanomiddleclick ignoredAppBundles -array com.apple.finder com.apple.Terminal
```

### Tap to click

The app can handle both clicks and trackpad taps.
By default, it follows the system behavior for 1 and 2 fingers, but you can set a custom one:

```sh
defaults write co.myrt.nanomiddleclick tapToClick -bool true
```

### Magic Mouse mode

Magic Mouse supports several types of actions to emulate a middle-click:

- `center` — click with one finger in the horizontal center zone. 
- `threeFinger` — click with 3 fingers anywhere
- `disabled` — ignore Magic Mouse and emulate middle-click only with trackpad.

Default is `center`.

```sh
defaults write co.myrt.nanomiddleclick mouseClickMode -string threeFinger
```

## Credits

Heavily inspired by [MiddleClick.app](https://github.com/artginzburg/MiddleClick)

## License

MIT.
