use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;

mod mio_channel;
mod mpv;
mod websocket;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    mpvsocket_path: PathBuf,

    #[arg(short('a'), long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    websocket_server_bind_address: IpAddr,

    #[arg(short('p'), visible_short_alias('w'), long, default_value_t = 6677)]
    websocket_server_port: u16,
}

fn main() {
    let args = Args::parse();

    println!(
        "Starting WebSocket server at `{}:{}`",
        args.websocket_server_bind_address, args.websocket_server_port
    );
    let server = websocket::Server::new(SocketAddr::new(
        args.websocket_server_bind_address,
        args.websocket_server_port,
    ))
    .start();

    println!(
        "Connecting to mpv socket at `{}`",
        args.mpvsocket_path.display()
    );
    mpv::Client::new(args.mpvsocket_path).poll_and_send_messages_to_server(server);
}
