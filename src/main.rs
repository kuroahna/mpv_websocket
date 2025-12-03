use clap::Parser;
use std::backtrace::Backtrace;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::panic::{self, PanicHookInfo};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::error;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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

    #[arg(short('s'), long, default_value_t = false)]
    secondary_subtitles: bool,
}

struct LazyFileLogger {
    log_dir: PathBuf,
    state: Mutex<Option<(NonBlocking, WorkerGuard)>>,
}

impl LazyFileLogger {
    fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            state: Mutex::new(None),
        }
    }
}

impl<'a> MakeWriter<'a> for LazyFileLogger {
    type Writer = NonBlocking;

    fn make_writer(&'a self) -> Self::Writer {
        let mut guard = self.state.lock().unwrap();

        if let Some((writer, _)) = guard.as_ref() {
            return writer.clone();
        }

        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("mpv_websocket")
            .filename_suffix(".txt")
            .build(&self.log_dir)
            .expect("Failed to create rolling file appender");

        let (non_blocking_writer, worker_guard) = tracing_appender::non_blocking(file_appender);
        *guard = Some((non_blocking_writer.clone(), worker_guard));
        non_blocking_writer
    }
}

fn main() {
    let log_dir = if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.pop();
        exe_path.join("logs")
    } else {
        PathBuf::from("logs")
    };

    let file_logger = LazyFileLogger::new(log_dir);
    tracing_subscriber::registry()
        .with(LevelFilter::WARN)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_logger) // <-- Use our lazy logger here
                .with_ansi(false),
        )
        .init();

    panic::set_hook(Box::new(|panic_info: &PanicHookInfo| {
        // <-- The type is now PanicHookInfo
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s
        } else {
            "Box<Any>"
        };

        error!(
            "SERVER PANIC: {message} at {location}\n\nBacktrace:\n{backtrace}",
            message = message,
            location = panic_info.location().unwrap(),
            backtrace = Backtrace::capture()
        );
    }));

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
    mpv::Client::new(args.mpvsocket_path, args.secondary_subtitles).poll_and_send_messages_to_server(server);
}
