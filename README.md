# mpv_websocket

## Description

A plugin for [mpv](https://mpv.io/) written in Rust that opens a WebSocket
locally on port `6677` by default and sends the subtitles to all connected
clients. The plugin uses mpv's
[JSON IPC](https://mpv.io/manual/master/#json-ipc) protocol for capturing the
subtitles.

On Linux/Mac, by specifying
[--input-ipc-server](https://mpv.io/manual/master/#options-input-ipc-server)
in the command line arguments or
[mpv.conf](https://mpv.io/manual/master/#files-~/-config/mpv/mpv-conf), a
[Unix Domain Socket](https://en.wikipedia.org/wiki/Unix_domain_socket) is
automatically created by mpv. On Windows, a
[Named Pipe](https://en.wikipedia.org/wiki/Named_pipe) is automatically created.
mpv_websocket will connect to the unix socket/named pipe and stream any
[sub-text](https://mpv.io/manual/master/#command-interface-sub-text) change
events to the WebSocket clients.

A WebSocket client such as
[texthooker-ui](https://github.com/Renji-XD/texthooker-ui)
can stream the text by the server and display it to your browser.

## Why?

Common solutions for sharing subtitles to the browser involves using the
clipboard. This is not very reliable because sometimes the contents do not
properly save to the clipboard, which requires the user to manually re-copy the
contents until it has properly been saved. Furthermore, when using a texthooker
page that listens for clipboard change events, it is not a friendly user
experience when you copy contents into your clipboard that you do not want to
show up in the texthooker page. This requires the user to manually delete the
unwanted copied text on the page.

## Build
If you want to build the binary yourself, you can follow the instructions below.
Otherwise, skip to the [Install](#install) section.

Ensure you have Rust installed. The installation instructions can be found
[here](https://www.rust-lang.org/learn/get-started). Then you can build the
binary with

```
cargo build --release
```

## Install

Pre-compiled binaries are available in the
[Releases](https://github.com/kuroahna/mpv_websocket/releases) page

### Windows
1. Copy the
   [mpv_websocket.exe](https://github.com/kuroahna/mpv_websocket/releases/latest/download/x86_64-pc-windows-gnu.zip)
   binary file into your
   [%appdata%\mpv](https://mpv.io/manual/stable/#files-on-windows) folder.
   Create the folder if it does not already exist
2. Copy [mpv.conf](mpv/mpv.conf) into your
   [%appdata%\mpv](https://mpv.io/manual/stable/#files-on-windows) folder
3. Copy
   [run_websocket_server_windows.lua](mpv/scripts/run_websocket_server_windows.lua)
   into your
   [%appdata%\mpv\scripts](https://mpv.io/manual/stable/#files-on-windows)
   folder. Create the folder if it does not already exist

<details><summary>Expected file structure</summary>

```
%appdata%\mpv
├── mpv.conf
├── mpv_websocket.exe
└── scripts
    └── run_websocket_server_windows.lua
```

</details>

### Linux
1. Copy the
   [mpv_websocket](https://github.com/kuroahna/mpv_websocket/releases/latest/download/x86_64-unknown-linux-musl.zip)
   binary file into your
   [~/.config/mpv](https://mpv.io/manual/stable/#files) folder. Create the
   folder if it does not already exist
2. Copy [mpv.conf](mpv/mpv.conf) into your
   [~/.config/mpv](https://mpv.io/manual/stable/#files) folder
3. Copy
   [run_websocket_server_linux.lua](mpv/scripts/run_websocket_server_linux.lua)
   into your
   [~/.config/mpv/scripts](https://mpv.io/manual/stable/#files) folder.
   Create the folder if it does not already exist

<details><summary>Expected file structure</summary>

```
~/.config/mpv/
├── mpv.conf
├── mpv_websocket
└── scripts
    └── run_websocket_server_linux.lua
```

</details>


### Mac
Note, I do not have a Mac and cannot test it, but it should be the same as Linux

1. Copy the
   [mpv_websocket](https://github.com/kuroahna/mpv_websocket/releases/latest/download/x86_64-apple-darwin.zip)
   binary file into your
   [~/.config/mpv](https://mpv.io/manual/stable/#files) folder. Create the
   folder if it does not already exist
2. Copy [mpv.conf](mpv/mpv.conf) into your
   [~/.config/mpv](https://mpv.io/manual/stable/#files) folder
3. Copy
   [run_websocket_server_linux.lua](mpv/scripts/run_websocket_server_linux.lua)
   into your
   [~/.config/mpv/scripts](https://mpv.io/manual/stable/#files) folder.
   Create the folder if it does not already exist

<details><summary>Expected file structure</summary>

```
~/.config/mpv/
├── mpv.conf
├── mpv_websocket
└── scripts
    └── run_websocket_server_linux.lua
```

</details>

## Troubleshooting
If after following the [Installation](#install) steps and mpv_websocket doesn't
seem to work:

- Double check that you have correctly installed the files in the correct
  folders for your platform. See the [Install](#install) guide for more details.

- Try running the mpv_websocket binary file in a terminal manually to see if
  there's any errors with running the server.

  To do this, first open mpv and play a video file with subtitles. If you have
  installed [mpv.conf](mpv/mpv.conf) correctly, then mpv should automatically
  create an IPC socket under `/tmp/mpv-socket` for Linux/Mac, or
  `\\.\pipe\tmp\mpv-socket` for Windows. The IPC socket is required for the
  mpv_websocket server to capture the subtitles from mpv.

  For Windows, open command prompt, and run
  ```
  %appdata%\mpv\mpv_websocket.exe -m \\.\pipe\tmp\mpv-socket -w 6677
  ```

  For Linux/Mac, open a terminal, and run
  ```
  ~/.config/mpv/mpv_websocket -m /tmp/mpv-socket -w 6677
  ```

  If there are no errors/output in the terminal/command prompt, then
  mpv_websocket is successfully running.

  If manually running the binary works but not when you simply open mpv, then
  double check and make sure that you have properly installed one of the
  [scripts](mpv/scripts) for your platform in the correct folder. See the
  [Install](#install) guide for more details. The lua script is how mpv
  automatically executes the mpv_websocket binary.

  If there are errors in the terminal/command prompt, there could be variety of
  errors such as

  ```
  > ~/.config/mpv/mpv_websocket -m /tmp/mpv-socket -w 6677

  thread 'main' panicked at 'Is mpv running with `--input-ipc-server=/tmp/mpv-socket`: Connection refused (os error 111)', src/mpv.rs:54:13
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
  thread '<unnamed>' panicked at 'called `Result::unwrap()` on an `Err` value: RecvError', src/websocket.rs:30:39
  ```

  This error indicates that mpv did not create the IPC socket. You should double
  check that [mpv.conf](mpv/mpv.conf) has been properly installed. You should
  also double check that mpv is running _before_ running the mpv_websocket
  binary in the terminal

  ```
  > ~/.config/mpv/mpv_websocket -m /tmp/mpv-socket -w 6677

  thread '<unnamed>' panicked at 'The address `0.0.0.0:6677` is in use: address in use', src/websocket.rs:27:37
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
  ```

  This error indicates that the address `0.0.0.0:6677` is already in use and the
  mpv_websocket server could not start on port 6677. Check to make sure you do
  not have any other servers running on this port. If you do, close them and try
  again. It is also possible that you have mpv_websocket already running and it
  did not automatically close, so double check if it is running via Task Manager
  (Windows) or `pgrep mpv_websocket` (Linux/Mac).

- Ensure you are using the
  [latest version of mpv](https://mpv.io/installation/).

  Note that for Linux, some package managers may distribute old versions
  of mpv. According to mpv's official documentation, it is recommended that
  you compile mpv using
  [mpv-build](https://github.com/mpv-player/mpv-build/),
  or use third party libraries instead.

- Try [manually building](#build) the binary instead of using the pre-compiled
  binary.

## Usage

After installing the plugin, when you play a video using mpv with subtitles, mpv
will automatically start the `mpv_websocket` server at `ws://localhost:6677` (or
the port you have specified in the script)

You will need a WebSocket client such as
[texthooker-ui](https://github.com/Renji-XD/texthooker-ui) to stream the
subtitles and display it to your browser.
