use std::{
    io,
    sync::{mpsc, Arc, Mutex},
};

use mio::{event::Source, Token, Waker};

pub fn sync_channel<T>(bound: u32) -> (SyncSender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::sync_channel(bound as usize);

    let waker = Arc::new(Mutex::new(None));

    (
        SyncSender {
            waker: waker.clone(),
            tx,
        },
        Receiver { waker, rx },
    )
}

#[derive(Clone)]
pub struct SyncSender<T> {
    waker: Arc<Mutex<Option<Waker>>>,
    tx: mpsc::SyncSender<T>,
}

impl<T> SyncSender<T> {
    pub fn send(&self, t: T) -> Result<(), mpsc::SendError<T>> {
        self.tx.send(t)?;

        if let Some(waker) = &*self.waker.lock().unwrap_or_else(|e| e.into_inner()) {
            waker.wake().expect("unable to wake");
        }

        Ok(())
    }
}

pub struct Receiver<T> {
    waker: Arc<Mutex<Option<Waker>>>,
    rx: mpsc::Receiver<T>,
}

impl<T> Receiver<T> {
    pub fn try_recv(&self) -> Result<T, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}

impl<T> Source for Receiver<T> {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: Token,
        _: mio::Interest,
    ) -> io::Result<()> {
        let mut waker = self.waker.lock().unwrap_or_else(|e| e.into_inner());
        if waker.is_none() {
            *waker = Some(Waker::new(registry, token)?);
        }
        Ok(())
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: Token,
        interests: mio::Interest,
    ) -> io::Result<()> {
        self.deregister(registry)?;
        self.register(registry, token, interests)?;
        Ok(())
    }

    fn deregister(&mut self, _: &mio::Registry) -> io::Result<()> {
        let mut waker = self.waker.lock().unwrap_or_else(|e| e.into_inner());
        *waker = None;
        Ok(())
    }
}
