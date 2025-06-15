use mio::event::Source;
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Token};
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt::Display;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use tracing::warn;
use tungstenite::protocol::Role;
use tungstenite::WebSocket;

use crate::mio_channel::{self, SyncSender};

const SERVER: Token = Token(0);
const BROADCAST: Token = Token(SERVER.0 + 1);

trait TokenExt {
    fn next(&self) -> Self;
}

impl TokenExt for Token {
    fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

trait Stream: Read + Write + Source {}

impl Stream for TcpStream {}

struct EmptyStream;

impl Stream for EmptyStream {}

impl Read for EmptyStream {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}

impl Write for EmptyStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Source for EmptyStream {
    fn register(&mut self, _: &mio::Registry, _: Token, _: Interest) -> io::Result<()> {
        Ok(())
    }

    fn reregister(&mut self, _: &mio::Registry, _: Token, _: Interest) -> io::Result<()> {
        Ok(())
    }

    fn deregister(&mut self, _: &mio::Registry) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
enum WebSocketError {
    Io(io::Error),
    Handshake(
        tungstenite::HandshakeError<
            tungstenite::ServerHandshake<
                Box<dyn Stream>,
                tungstenite::handshake::server::NoCallback,
            >,
        >,
    ),
    WebSocket(tungstenite::Error),
}

impl Display for WebSocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSocketError::Io(error) => write!(f, "IO error: {error}"),
            WebSocketError::Handshake(error) => write!(f, "handshake error: {error}"),
            WebSocketError::WebSocket(error) => write!(f, "WebSocket error: {error}"),
        }
    }
}

impl Error for WebSocketError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WebSocketError::Io(error) => Some(error),
            WebSocketError::Handshake(error) => Some(error),
            WebSocketError::WebSocket(error) => Some(error),
        }
    }
}

impl From<io::Error> for WebSocketError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl
    From<
        tungstenite::HandshakeError<
            tungstenite::ServerHandshake<
                Box<dyn Stream>,
                tungstenite::handshake::server::NoCallback,
            >,
        >,
    > for WebSocketError
{
    fn from(
        value: tungstenite::HandshakeError<
            tungstenite::ServerHandshake<
                Box<dyn Stream>,
                tungstenite::handshake::server::NoCallback,
            >,
        >,
    ) -> Self {
        Self::Handshake(value)
    }
}

impl From<tungstenite::Error> for WebSocketError {
    fn from(value: tungstenite::Error) -> Self {
        Self::WebSocket(value)
    }
}

enum WebSocketMessage {
    UpgradeWebSocket(Box<dyn Stream>),
    MessagesAvailable,
    CanWrite,
    SendText(Arc<str>),
}

enum WebSocketState {
    Unconnected(UnconnectedState),
    Connected(ConnectedState),
    Closed(WebSocket<Box<dyn Stream>>),
}

impl WebSocketState {
    fn next_state(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        match self {
            WebSocketState::Unconnected(state) => *self = state.next_state(message)?,
            WebSocketState::Connected(state) => {
                if let Some(state) = state.next_state(message)? {
                    *self = state;
                }
            }
            WebSocketState::Closed(_) => {
                // This can happen if multiple events are processed for a closed socket.
                // It's safe to ignore them.
            }
        }

        Ok(())
    }
}

struct UnconnectedState;

impl UnconnectedState {
    fn next_state(&mut self, message: WebSocketMessage) -> Result<WebSocketState, WebSocketError> {
        match message {
            WebSocketMessage::UpgradeWebSocket(stream) => {
                Ok(WebSocketState::Connected(ConnectedState {
                    websocket: tungstenite::accept(stream)?,
                    messages: VecDeque::new(),
                    write: WriteState::Unwritable,
                }))
            }
            WebSocketMessage::MessagesAvailable => {
                panic!("messages available on an unconnected WebSocket")
            }
            WebSocketMessage::CanWrite => panic!("writable event on an unconnected WebSocket"),
            WebSocketMessage::SendText(_) => panic!("text sent on an unconnected WebSocket"),
        }
    }
}

enum WriteState {
    Unwritable,
    Writable,
}

struct ConnectedState {
    websocket: WebSocket<Box<dyn Stream>>,
    messages: VecDeque<Arc<str>>,
    write: WriteState,
}

impl ConnectedState {
    fn transition_to_closed(&mut self) -> Result<Option<WebSocketState>, WebSocketError> {
        let state = std::mem::replace(
            self,
            ConnectedState {
                websocket: WebSocket::from_raw_socket(Box::new(EmptyStream), Role::Server, None),
                messages: VecDeque::new(),
                write: WriteState::Unwritable,
            },
        );
        Ok(Some(WebSocketState::Closed(state.websocket)))
    }

    fn next_state(
        &mut self,
        message: WebSocketMessage,
    ) -> Result<Option<WebSocketState>, WebSocketError> {
        match message {
            WebSocketMessage::UpgradeWebSocket(_) => {
                panic!("connection is already upgraded to a WebSocket")
            }
            WebSocketMessage::MessagesAvailable => loop {
                match self.websocket.read() {
                    Ok(msg) => msg,
                    Err(e) => match e {
                        tungstenite::Error::ConnectionClosed
                        | tungstenite::Error::Protocol(
                            tungstenite::error::ProtocolError::ResetWithoutClosingHandshake
                            | tungstenite::error::ProtocolError::InvalidCloseSequence
                            | tungstenite::error::ProtocolError::UnmaskedFrameFromClient,
                        ) => {
                            return self.transition_to_closed();
                        }
                        tungstenite::Error::Io(ref error) => match error.kind() {
                            io::ErrorKind::WouldBlock => return Ok(None),
                            io::ErrorKind::Interrupted => continue,
                            io::ErrorKind::ConnectionReset => return self.transition_to_closed(),
                            _ => {
                                eprintln!(
                                    "unhandled websocket read io error, closing connection: {e}"
                                );
                                warn!(
                                    "unhandled websocket read io error, closing connection: {}",
                                    e
                                );
                                return self.transition_to_closed();
                            }
                        },
                        _ => {
                            eprintln!("unhandled websocket read error, closing connection: {e}");
                            warn!("unhandled websocket read error, closing connection: {}", e);
                            return self.transition_to_closed();
                        }
                    },
                };
            },
            WebSocketMessage::CanWrite => {
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
            WebSocketMessage::SendText(message) => {
                self.messages.push_back(message);

                if let WriteState::Unwritable = self.write {
                    return Ok(None);
                }

                self.send_message()
            }
        }
    }

    fn send_message(&mut self) -> Result<Option<WebSocketState>, WebSocketError> {
        if let Some(msg) = self.messages.pop_front() {
            if let Err(e) = self
                .websocket
                .send(tungstenite::Message::Text((*msg).into()))
            {
                match e {
                    tungstenite::Error::ConnectionClosed
                    | tungstenite::Error::Protocol(
                        tungstenite::error::ProtocolError::ResetWithoutClosingHandshake
                        | tungstenite::error::ProtocolError::InvalidCloseSequence
                        | tungstenite::error::ProtocolError::UnmaskedFrameFromClient,
                    ) => {
                        return self.transition_to_closed();
                    }
                    tungstenite::Error::Io(ref err) => match err.kind() {
                        // On write error, tungstenite will store the frame in
                        // its internal buffer and send it on a subsequent call
                        // to write or flush. Hence, we do not need to push the
                        // message back into our buffer here
                        io::ErrorKind::WouldBlock => self.write = WriteState::Unwritable,
                        io::ErrorKind::Interrupted => {}
                        io::ErrorKind::ConnectionReset => return self.transition_to_closed(),
                        _ => {
                            eprintln!(
                                "unhandled websocket write io error, closing connection: {e}"
                            );
                            warn!(
                                "unhandled websocket write io error, closing connection: {}",
                                e
                            );
                            return self.transition_to_closed();
                        }
                    },
                    _ => {
                        eprintln!("unhandled websocket write error, closing connection: {e}");
                        warn!("unhandled websocket write error, closing connection: {}", e);
                        return self.transition_to_closed();
                    }
                }
            }
        }

        Ok(None)
    }
}

pub struct Server {
    address: SocketAddr,
}

pub struct ServerStarted {
    sender: SyncSender<Arc<str>>,
}

impl Server {
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
    }

    pub fn start(self) -> ServerStarted {
        let (sender, mut receiver) = mio_channel::sync_channel::<Arc<str>>(10);
        let mut poll =
            Poll::new().unwrap_or_else(|e| panic!("failed to create poll instance: {e:?}"));
        let mut events = Events::with_capacity(128);

        let mut server = TcpListener::bind(self.address)
            .unwrap_or_else(|e| panic!("failed to bind address `{}`: {:?}", self.address, e));

        poll.registry()
            .register(&mut server, SERVER, Interest::READABLE)
            .unwrap_or_else(|e| panic!("failed to register server to poll instance: {e:?}"));
        poll.registry()
            .register(&mut receiver, BROADCAST, Interest::READABLE)
            .unwrap_or_else(|e| {
                panic!(
                    "failed to register broadcast channel to poll instance: {e:?}"
                )
            });

        thread::spawn(move || {
            let mut token_to_tcpstreams = HashMap::new();
            let mut token_to_websockets: HashMap<Token, WebSocketState> = HashMap::new();
            let mut unique_token = Token(BROADCAST.0);

            loop {
                if let Err(e) = poll.poll(&mut events, None) {
                    if e.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    panic!("failed to poll for events: {e:?}");
                }

                for event in &events {
                    match event.token() {
                        SERVER => {
                            if !event.is_readable() {
                                continue;
                            }

                            loop {
                                let (mut stream, address) = match server.accept() {
                                    Ok((stream, address)) => (stream, address),
                                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                        break;
                                    }
                                    Err(e) => {
                                        // This is a server-wide error, not a client error.
                                        // Panicking is acceptable as per the requirements.
                                        panic!("failed to accept connection: {e:?}");
                                    }
                                };

                                unique_token = unique_token.next();
                                if let Err(e) = poll.registry().register(
                                    &mut stream,
                                    unique_token,
                                    Interest::READABLE.add(Interest::WRITABLE),
                                ) {
                                    eprintln!(
                                        "failed to register incoming connection `{address}` for events: {e:?}. Connection closed."
                                    );
                                    warn!(
                                        "failed to register incoming connection `{}` for events: {:?}. Connection closed.",
                                        address, e
                                    );
                                    continue;
                                }

                                token_to_tcpstreams.insert(unique_token, stream);
                            }
                        }
                        BROADCAST => {
                            if !event.is_readable() {
                                continue;
                            }

                            if let Ok(msg) = receiver.try_recv() {
                                let mut closed_connection_tokens = Vec::new();
                                for (token, state) in &mut token_to_websockets {
                                    if let Err(e) =
                                        state.next_state(WebSocketMessage::SendText(msg.clone()))
                                    {
                                        eprintln!("failed to send text `{msg}` to WebSocket with token {token:?}: {e:?}. Connection will be closed.");
                                        warn!("failed to send text `{}` to WebSocket with token {:?}: {:?}. Connection will be closed.", msg, token, e);
                                    }
                                    if let WebSocketState::Closed(_) = state {
                                        closed_connection_tokens.push(*token);
                                    }
                                }

                                for token in closed_connection_tokens {
                                    let state = token_to_websockets
                                        .remove(&token)
                                        .expect("WebSocket should not have been removed yet");
                                    let WebSocketState::Closed(mut stream) = state else {
                                        panic!("all WebSocket connections should be closed");
                                    };
                                    if let Err(e) = poll.registry().deregister(stream.get_mut()) {
                                        eprintln!(
                                            "failed to deregister stream for token {token:?}: {e:?}"
                                        );
                                        warn!(
                                            "failed to deregister stream for token {:?}: {:?}",
                                            token, e
                                        );
                                    }
                                }
                            }
                        }
                        token => {
                            if event.is_readable() {
                                if let Some(stream) = token_to_tcpstreams.remove(&token) {
                                    let mut state =
                                        WebSocketState::Unconnected(UnconnectedState);
                                    if let Err(e) = state.next_state(
                                        WebSocketMessage::UpgradeWebSocket(Box::new(stream)),
                                    ) {
                                        eprintln!("failed to upgrade tcp stream to WebSocket for token {token:?}: {e:?}. Connection closed.");
                                        warn!("failed to upgrade tcp stream to WebSocket for token {:?}: {:?}. Connection closed.", token, e);
                                        continue;
                                    }

                                    // There is no guarantee that another
                                    // readiness event will be delivered
                                    // until the readiness event has been
                                    // drained
                                    if let Err(e) =
                                        state.next_state(WebSocketMessage::MessagesAvailable)
                                    {
                                        eprintln!("failed to read messages on new WebSocket with token {token:?}: {e:?}");
                                        warn!("failed to read messages on new WebSocket with token {:?}: {:?}", token, e);
                                    }

                                    if let WebSocketState::Closed(mut stream) = state {
                                        if let Err(e) =
                                            poll.registry().deregister(stream.get_mut())
                                        {
                                            eprintln!("failed to deregister stream for token {token:?}: {e:?}");
                                            warn!("failed to deregister stream for token {:?}: {:?}", token, e);
                                        }
                                    } else {
                                        token_to_websockets.insert(token, state);
                                    }
                                } else {
                                    let mut needs_removal = false;
                                    if let Some(state) = token_to_websockets.get_mut(&token) {
                                        if let Err(e) = state
                                            .next_state(WebSocketMessage::MessagesAvailable)
                                        {
                                            eprintln!("failed to read messages on WebSocket with token {token:?}: {e:?}");
                                            warn!("failed to read messages on WebSocket with token {:?}: {:?}", token, e);
                                        }
                                        if matches!(state, WebSocketState::Closed(_)) {
                                            needs_removal = true;
                                        }
                                    }

                                    if needs_removal {
                                        if let Some(WebSocketState::Closed(mut stream)) =
                                            token_to_websockets.remove(&token)
                                        {
                                            if let Err(e) =
                                                poll.registry().deregister(stream.get_mut())
                                            {
                                                eprintln!("failed to deregister stream for token {token:?}: {e:?}");
                                                warn!("failed to deregister stream for token {:?}: {:?}", token, e);
                                            }
                                        }
                                    }
                                }
                            }

                            if event.is_writable() {
                                let mut needs_removal = false;
                                if let Some(state) = token_to_websockets.get_mut(&token) {
                                    if let Err(e) = state.next_state(WebSocketMessage::CanWrite) {
                                        eprintln!("failed to handle writable event on WebSocket with token {token:?}: {e:?}");
                                        warn!("failed to handle writable event on WebSocket with token {:?}: {:?}", token, e);
                                    }
                                    if matches!(state, WebSocketState::Closed(_)) {
                                        needs_removal = true;
                                    }
                                }

                                if needs_removal {
                                    if let Some(WebSocketState::Closed(mut stream)) =
                                        token_to_websockets.remove(&token)
                                    {
                                        if let Err(e) = poll.registry().deregister(stream.get_mut())
                                        {
                                            eprintln!(
                                                "failed to deregister stream for token {token:?}: {e:?}"
                                            );
                                            warn!(
                                                "failed to deregister stream for token {:?}: {:?}",
                                                token, e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        ServerStarted { sender }
    }
}

impl ServerStarted {
    pub fn send_message(&self, message: Arc<str>) {
        self.sender.send(message.clone()).unwrap_or_else(|e| {
            panic!(
                "failed to send text `{message}` to WebSocket clients: {e:?}"
            )
        });
    }
}
