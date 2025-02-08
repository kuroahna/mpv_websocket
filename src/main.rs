use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;

mod mpv;
mod websocket;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    mpvsocket_path: PathBuf,

    #[arg(short('a'), long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    websocket_server_bind_address: IpAddr,

    #[arg(short, long, default_value_t = 6677)]
    websocket_server_port: u16,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let server = websocket::Server::new(SocketAddr::new(
        args.websocket_server_bind_address,
        args.websocket_server_port,
    ))
    .start();

    mpv::Client::new(args.mpvsocket_path)
        .connect()
        .await
        .poll_and_send_messages_to_server(server)
        .await;
}
