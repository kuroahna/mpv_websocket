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
```
cargo build
```

## Install

Pre-compiled binaries are available in the
[Releases](https://github.com/kuroahna/mpv_websocket/releases) page

### Windows
1. Copy `mpv_websocket.exe` into your
   [%appdata%\mpv](https://mpv.io/manual/stable/#files-on-windows) folder.
   Create the folder if it does not already exist
2. Copy [mpv.conf](mpv/mpv.conf) into your
   [%appdata%\mpv](https://mpv.io/manual/stable/#files-on-windows) folder
3. Copy
   [run_websocket_server_windows.lua](mpv/scripts/run_websocket_server_windows.lua)
   into your
   [%appdata%\mpv\scripts](https://mpv.io/manual/stable/#files-on-windows)
   folder. Create the folder if it does not already exist

### Linux
1. Copy `mpv_websocket` into your
   [~/.config/mpv](https://mpv.io/manual/stable/#files) folder. Create the
   folder if it does not already exist
2. Copy [mpv.conf](mpv/mpv.conf) into your
   [~/.config/mpv](https://mpv.io/manual/stable/#files) folder
3. Copy
   [run_websocket_server_linux.lua](mpv/scripts/run_websocket_server_linux.lua)
   into your
   [~/.config/mpv/scripts](https://mpv.io/manual/stable/#files) folder.
   Create the folder if it does not already exist

### Mac
Note, I do not have a Mac and cannot test it, but it should be the same as Linux

1. Copy `mpv_websocket` into your
   [~/.config/mpv](https://mpv.io/manual/stable/#files) folder. Create the
   folder if it does not already exist
2. Copy [mpv.conf](mpv/mpv.conf) into your
   [~/.config/mpv](https://mpv.io/manual/stable/#files) folder
3. Copy
   [run_websocket_server_linux.lua](mpv/scripts/run_websocket_server_linux.lua)
   into your
   [~/.config/mpv/scripts](https://mpv.io/manual/stable/#files) folder.
   Create the folder if it does not already exist

## Usage

After installing the plugin, when you play a video using mpv with subtitles, mpv
will automatically start the `mpv_websocket` server at `ws://0.0.0.0:6677` (or
the port you have specified in the script)

You will need a WebSocket client such as
[texthooker-ui](https://github.com/Renji-XD/texthooker-ui) to stream the
subtitles and display it to your browser.
