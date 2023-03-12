use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;

pub struct Server {
    address: SocketAddr,
}

pub struct ServerStarted {
    sender: Sender<String>,
}

impl Server {
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
    }

    pub fn start(self) -> ServerStarted {
        let (sender, receiver) = mpsc::channel();
        let server = ws::WebSocket::new(move |_| move |_| Ok(()))
            .expect("The WebSocket factory method returns Ok()");
        let broadcaster = server.broadcaster();
        thread::spawn(move || {
            server
                .listen(self.address)
                .unwrap_or_else(|e| panic!("The address `{}` is in use: {e}", self.address))
        });
        thread::spawn(move || loop {
            let msg = receiver.recv().unwrap();
            broadcaster.broadcast(msg).unwrap();
        });

        ServerStarted { sender }
    }
}

impl ServerStarted {
    pub fn send_message(&self, message: String) {
        self.sender.send(message).unwrap();
    }
}
