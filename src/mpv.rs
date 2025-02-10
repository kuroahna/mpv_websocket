use std::path::PathBuf;
use std::sync::Arc;

use mio::event::Source;
#[cfg(unix)]
use mio::net::UnixStream;
#[cfg(windows)]
use mio::windows::NamedPipe;
use mio::{Events, Interest, Poll, Token};
use serde::Deserialize;
use std::collections::VecDeque;
use std::error::Error;
use std::fmt::Display;
use std::io::{self, Read, Write};
#[cfg(windows)]
use std::path::Path;

use crate::mio_channel::SyncSender;
use crate::{mio_channel, websocket};

const CLIENT: Token = Token(0);
const BROADCAST: Token = Token(CLIENT.0 + 1);

#[cfg(windows)]
fn create_named_pipe<P: AsRef<Path>>(path: P) -> Result<NamedPipe, std::io::Error> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{FromRawHandle, IntoRawHandle};

    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED;

    let mut opts = OpenOptions::new();
    opts.read(true)
        .write(true)
        .custom_flags(FILE_FLAG_OVERLAPPED);
    let file = opts.open(path)?;
    // SAFETY: mpv should have created the named pipe automatically, provided
    // the user has properly started mpv with the `--input-ipc-server` option
    unsafe { Ok(NamedPipe::from_raw_handle(file.into_raw_handle())) }
}

trait Stream: Read + Write + Source {}

#[cfg(unix)]
impl Stream for UnixStream {}

#[cfg(windows)]
impl Stream for NamedPipe {}

struct EmptyStream;

impl Stream for EmptyStream {}

impl Read for EmptyStream {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

impl Write for EmptyStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Source for EmptyStream {
    fn register(&mut self, _: &mio::Registry, _: Token, _: Interest) -> std::io::Result<()> {
        Ok(())
    }

    fn reregister(&mut self, _: &mio::Registry, _: Token, _: Interest) -> std::io::Result<()> {
        Ok(())
    }

    fn deregister(&mut self, _: &mio::Registry) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
enum SocketError {
    Io(std::io::Error),
}

impl Display for SocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketError::Io(error) => write!(f, "IO error: {}", error),
        }
    }
}

impl Error for SocketError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SocketError::Io(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for SocketError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

enum SocketMessage {
    MessagesAvailable,
    CanWrite,
    SendText(Arc<str>),
}

enum SocketState {
    Connected(ConnectedState),
    Closed(Box<dyn Stream>),
}

impl SocketState {
    fn next_state(&mut self, message: SocketMessage) -> Result<(), SocketError> {
        match self {
            SocketState::Connected(state) => {
                if let Some(state) = state.next_state(message)? {
                    *self = state;
                }
            }
            SocketState::Closed(_) => panic!("socket is already closed"),
        }

        Ok(())
    }
}

enum WriteState {
    Unwritable,
    Writable,
}

struct ConnectedState {
    stream: Box<dyn Stream>,
    messages: VecDeque<Arc<str>>,
    write: WriteState,
    sender: SyncSender<Arc<str>>,
}

impl ConnectedState {
    fn next_state(&mut self, message: SocketMessage) -> Result<Option<SocketState>, SocketError> {
        match message {
            SocketMessage::MessagesAvailable => {
                let mut buffer = Vec::new();
                let mut internal_buffer = [0; 8192];

                loop {
                    match self.stream.read(&mut internal_buffer) {
                        Ok(0) => {
                            let (sender, _) = mio_channel::sync_channel::<Arc<str>>(1);
                            let state = std::mem::replace(
                                self,
                                ConnectedState {
                                    stream: Box::new(EmptyStream),
                                    messages: VecDeque::new(),
                                    write: WriteState::Unwritable,
                                    sender,
                                },
                            );
                            return Ok(Some(SocketState::Closed(state.stream)));
                        }
                        Ok(n) => {
                            buffer.extend_from_slice(&internal_buffer[..n]);
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(e) => return Err(From::from(e)),
                    }

                    let last_byte = match buffer.last() {
                        Some(last_byte) => *last_byte,
                        None => UTF8_NULL_CHARACTER,
                    };
                    // mpv ends each response with a newline.
                    //
                    // The buffer may not be completely filled with the full
                    // response, so we should continue reading
                    if last_byte == UTF8_NEWLINE_CHARACTER {
                        break;
                    }
                }

                let responses =
                    std::str::from_utf8(&buffer).expect("mpv should respond with UTF-8 strings");

                // There may be multiple responses in the buffer, separated by a
                // newline
                for line in responses.lines() {
                    let event = match serde_json::from_str::<PropertyChangeEvent>(line) {
                        Ok(event) => event,
                        Err(_) => {
                            // mpv sends other event changes in the socket that
                            // we don't care about
                            continue;
                        }
                    };

                    if event.data.is_empty() {
                        continue;
                    }

                    let data: Arc<str> = event.data.into();
                    self.sender.send(data.clone()).unwrap_or_else(|e| {
                        panic!(
                            "failed to send text `{}` to WebSocket clients: {:?}",
                            data, e
                        )
                    });
                }

                Ok(None)
            }
            SocketMessage::CanWrite => {
                if let WriteState::Unwritable = self.write {
                    self.write = WriteState::Writable;
                }

                // On write events, we send one message at a time because mio
                // will send another write event after each successful flush.
                // This will allow us to drain our internal buffer if there are
                // any remaining messages left. It also allows the caller to
                // respond to each message since it is done one at a time
                self.send_message()
            }
            SocketMessage::SendText(message) => {
                self.messages.push_back(message);

                if let WriteState::Unwritable = self.write {
                    return Ok(None);
                }

                self.send_message()
            }
        }
    }

    fn send_message(&mut self) -> Result<Option<SocketState>, SocketError> {
        if let Some(msg) = self.messages.pop_front() {
            if let Err(e) = self.stream.write_all(msg.as_bytes()) {
                match e.kind() {
                    io::ErrorKind::WriteZero => {
                        let (sender, _) = mio_channel::sync_channel::<Arc<str>>(1);
                        let state = std::mem::replace(
                            self,
                            ConnectedState {
                                stream: Box::new(EmptyStream),
                                messages: VecDeque::new(),
                                write: WriteState::Unwritable,
                                sender,
                            },
                        );
                        return Ok(Some(SocketState::Closed(state.stream)));
                    }
                    io::ErrorKind::WouldBlock => {
                        self.write = WriteState::Unwritable;
                        self.messages.push_front(msg);
                    }
                    io::ErrorKind::Interrupted => {
                        self.messages.push_front(msg);
                    }
                    _ => return Err(From::from(e)),
                }
            }
        }

        Ok(None)
    }
}

// The "1" in the command is the event id that will be sent back to us on the socket
// Example response:
// {"event":"property-change","id":1,"name":"sub-text","data":"hello world"}
const OBSERVE_PROPERTY_SUB_TEXT: &[u8; 46] =
    b"{\"command\":[\"observe_property\",1,\"sub-text\"]}\n";

const UTF8_NULL_CHARACTER: u8 = 0;
const UTF8_NEWLINE_CHARACTER: u8 = b"\n"[0];

#[derive(Deserialize)]
enum EventType {
    #[serde(rename = "property-change")]
    PropertyChange,
}

#[derive(Deserialize)]
enum Property {
    #[serde(rename = "sub-text")]
    SubText,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct PropertyChangeEvent {
    event: EventType,
    id: u32,
    name: Property,
    data: String,
}

pub struct Client {
    path: PathBuf,
}

impl Client {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn poll_and_send_messages_to_server(&mut self, server: websocket::ServerStarted) {
        let (sender, mut receiver) = mio_channel::sync_channel::<Arc<str>>(10);

        let mut poll =
            Poll::new().unwrap_or_else(|e| panic!("failed to create poll instance: {:?}", e));
        let mut events = Events::with_capacity(128);

        #[cfg(unix)]
        let mut stream = loop {
            match UnixStream::connect(&self.path) {
                Ok(stream) => break stream,
                // UnixStream::connect may return a WouldBlock in which case the
                // socket connection cannot be completed immediately. Usually it
                // means the backlog is full.
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!(
                    "is mpv running with `--input-ipc-server={}`: {:?}",
                    self.path
                        .to_str()
                        .expect("the socket path should be set by the user"),
                    e
                ),
            }
        };

        #[cfg(windows)]
        let mut stream = create_named_pipe(&self.path).unwrap_or_else(|e| {
            panic!(
                "is mpv running with `--input-ipc-server={}`: {:?}",
                self.path
                    .to_str()
                    .expect("the socket path should be set by the user"),
                e
            )
        });

        poll.registry()
            .register(
                &mut stream,
                CLIENT,
                Interest::READABLE.add(Interest::WRITABLE),
            )
            .unwrap_or_else(|e| {
                panic!("failed to register socket client to poll instance: {:?}", e)
            });
        poll.registry()
            .register(&mut receiver, BROADCAST, Interest::READABLE)
            .unwrap_or_else(|e| {
                panic!(
                    "failed to register broadcast channel to poll instance: {:?}",
                    e
                )
            });

        let mut state = SocketState::Connected(ConnectedState {
            stream: Box::new(stream),
            messages: VecDeque::new(),
            write: WriteState::Unwritable,
            sender,
        });
        state
            .next_state(SocketMessage::SendText(
                std::str::from_utf8(OBSERVE_PROPERTY_SUB_TEXT)
                    .expect("observe property sub-text command should be a valid UTF-8 string")
                    .into(),
            ))
            .unwrap_or_else(|e| panic!("message should not have been sent yet: {:?}", e));

        loop {
            if let Err(e) = poll.poll(&mut events, None) {
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                panic!("failed to poll for events: {:?}", e);
            }

            for event in events.iter() {
                match event.token() {
                    CLIENT => {
                        if event.is_readable() {
                            state
                                .next_state(SocketMessage::MessagesAvailable)
                                .unwrap_or_else(|e| {
                                    panic!("failed to read messages on socket: {:?}", e)
                                });
                            if let SocketState::Closed(mut stream) = state {
                                poll.registry().deregister(&mut stream).unwrap_or_else(|e| {
                                    panic!("failed to deregister stream: {:?}", e)
                                });
                                return;
                            }
                        }

                        if event.is_writable() {
                            state
                                .next_state(SocketMessage::CanWrite)
                                .unwrap_or_else(|e| {
                                    panic!("failed to handle writable event on socket: {:?}", e)
                                });
                            if let SocketState::Closed(mut stream) = state {
                                poll.registry().deregister(&mut stream).unwrap_or_else(|e| {
                                    panic!("failed to deregister stream: {:?}", e)
                                });
                                return;
                            }
                        }
                    }
                    BROADCAST => {
                        if !event.is_readable() {
                            continue;
                        }

                        if let Ok(msg) = receiver.try_recv() {
                            server.send_message(msg);
                        }
                    }
                    _ => unreachable!("only the client and broadcast channel should be registered"),
                }
            }
        }
    }
}
