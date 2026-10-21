# LMP

An audio player for macOS and Windows, written in Rust with [egui](https://github.com/emilk/egui) / eframe.
Plays MP3, FLAC, WAV, OGG, M4A and AAC, with a track list, shuffle, repeat one / repeat all,
and album covers.

## Running in development

```sh
cargo run
```

## Building

Both builds are made on macOS, including the Windows one, which is cross-compiled with MinGW.

### Prerequisites

```sh
cargo install cargo-bundle                    # builds the .app and .dmg
rustup target add x86_64-pc-windows-gnu       # Windows target
brew install mingw-w64                        # Windows linker (see .cargo/config.toml)
brew install makensis                         # Windows installer
```

### macOS

```sh
cargo bundle --release
```

Output:

- `target/release/bundle/osx/LMP.app` — the app itself;
- `target/release/bundle/dmg/LMP.dmg` — disk image for distribution: it contains
  the app and a shortcut to the Applications folder, so you install by dragging the app there.

The build is Apple Silicon (arm64) only.

### Windows

```sh
cargo build --release --target x86_64-pc-windows-gnu   # the app
mkdir -p target/installer
makensis -DVERSION=0.1.0 installer/windows.nsi         # the installer
```

Output:

- `target/x86_64-pc-windows-gnu/release/lmp.exe` — the app, runs without installation;
- `target/installer/LMP Setup 0.1.0.exe` — the installer: installs into Program Files,
  creates Start menu shortcuts and an optional desktop shortcut, and registers the app
  in "Apps & features" so it can be uninstalled.

The icon is embedded into the `.exe` at build time (`build.rs`); the installer script is
`installer/windows.nsi`.

### Releasing a new version

The version number lives in two places and has to be changed in both:

- `version` in `Cargo.toml`;
- `-DVERSION=` in the `makensis` command.

### Code signing

The builds are not signed:

- **macOS** — on another Mac, Gatekeeper will say the app can't be opened.
  To open it: right-click → Open.
- **Windows** — SmartScreen will show "Windows protected your PC".
  To run it: More info → Run anyway.

Getting rid of these warnings requires signing: an Apple Developer ID plus notarization on macOS,
and a code-signing certificate on Windows.
