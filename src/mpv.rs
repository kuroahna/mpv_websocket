use std::path::PathBuf;
use serde_json::Value;

use parity_tokio_ipc::{Connection, Endpoint};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(feature = "talk-tts")]
use tts::*;

use crate::websocket;

// The "1" in the command is the event id that will be sent back to us on the socket
// Example response:
// {"event":"property-change","id":1,"name":"sub-text","data":"hello world"}
const OBSERVE_PROPERTY_SUB_TEXT: &[u8; 46] =
    b"{\"command\":[\"observe_property\",1,\"sub-text\"]}\n";

const UTF8_NULL_CHARACTER: u8 = 0;
const UTF8_NEWLINE_CHARACTER: u8 = b"\n"[0];

#[derive(Deserialize, PartialEq)]
enum EventType {
    #[serde(rename = "property-change")]
    PropertyChange,
}

#[derive(Deserialize, PartialEq)]
enum Property {
    #[serde(rename = "sub-text")]
    SubText,
}

#[derive(Deserialize, PartialEq)]
struct PropertyChangeEvent {
    event: EventType,
    id: u32,
    name: Property,
    data: Option<Value>,
}

pub struct Client {
    path: PathBuf,
}

pub struct ConnectedClient {
    path: PathBuf,
    connection: Connection,
}

impl Client {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub async fn connect(self) -> ConnectedClient {
        let mut connection = Endpoint::connect(&self.path).await.unwrap_or_else(|e| {
            panic!(
                "Is mpv running with `--input-ipc-server={}`: {e}",
                self.path
                    .to_str()
                    .expect("The socket path should be set by the user")
            )
        });

        connection
            .write_all(OBSERVE_PROPERTY_SUB_TEXT)
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "Could not write to socket at `{}`: {e}",
                    self.path
                        .to_str()
                        .expect("The socket path should be set by the user")
                )
            });

        ConnectedClient {
            path: self.path,
            connection,
        }
    }
}

impl ConnectedClient {
    pub async fn poll_and_send_messages_to_server(&mut self, server: websocket::ServerStarted) {
        #[cfg(feature = "talk-tts")]
        let mut tts = Tts::default().unwrap();
        let mut buffer = Vec::new();
        loop {
            self.connection
                .read_buf(&mut buffer)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "Could not read socket at `{}`: {e}",
                        self.path
                            .to_str()
                            .expect("The socket path should be set by the user")
                    )
                });

            let last_byte = match buffer.last() {
                Some(last_byte) => *last_byte,
                None => UTF8_NULL_CHARACTER,
            };
            // mpv ends each response with a newline
            // The buffer may not be completely filled with the full response, so we should continue reading
            if last_byte != UTF8_NEWLINE_CHARACTER {
                continue;
            }
            let responses =
                std::str::from_utf8(&buffer).expect("mpv should respond with UTF-8 strings");

            // There may be multiple responses in the buffer, separated by a newline
            for line in responses.lines() {
                let event = match serde_json::from_str::<PropertyChangeEvent>(line) {
                    Ok(event) => event,
                    Err(_) => {
                        // mpv sends other event changes in the socket that we don't care about
                        continue;
                    }
                };

                if event.data.is_empty() {
                    continue;
                }

                if event.name==crate::mpv::Property::SubText  {
                    if let Some(data) = &event.data {
                        let speak = data.to_string();
                        if speak.len()>2 {
                            println!("{}",&speak);
                            #[cfg(feature = "talk-tts")]
                            tts.speak(&speak.replace("\\n"," "),false).ok();
                        server.send_message(speak);
                        }
                    }
                }
            }

            buffer.clear();
        }
    }
}
