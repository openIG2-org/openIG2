//! The links lockstep messages travel over (ADR 0007 §6): the in-process loopback now, QUIC (ADR 0005) later
//! behind the same trait.

use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

use crate::lockstep::Msg;
use crate::wire;

/// One end of a point-to-point link carrying [`Msg`]s in order.
pub trait Transport {
    /// Queue `msg` for the other end (a closed link drops it: the other end has left).
    fn send(&mut self, msg: &Msg);
    /// The next message received, if any.
    fn recv(&mut self) -> Result<Option<Msg>, wire::Error>;
    /// A value both ends of this link share and a man in the middle can't (QUIC: the TLS exporter); the lobby's
    /// password check binds to it (ADR 0008 §3.1). An in-process link has nobody in the middle.
    fn binding(&self) -> [u8; 32] {
        [0; 32]
    }
    /// The other end is known to be gone.
    fn closed(&self) -> bool {
        false
    }
}

/// An in-process link. Messages cross it encoded, as they would cross a network, and the ends may live on
/// different threads. Dropping one end closes the link: the other end reports [`Transport::closed`] once it has
/// received everything sent before.
pub struct Loopback {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    closed: bool,
}

/// The two ends of a new loopback link.
pub fn loopback() -> (Loopback, Loopback) {
    let (a_tx, b_rx) = channel();
    let (b_tx, a_rx) = channel();
    (Loopback { tx: a_tx, rx: a_rx, closed: false }, Loopback { tx: b_tx, rx: b_rx, closed: false })
}

impl Transport for Loopback {
    fn send(&mut self, msg: &Msg) {
        if self.tx.send(wire::to_vec(msg).expect("messages encode")).is_err() {
            self.closed = true;
        }
    }

    fn recv(&mut self) -> Result<Option<Msg>, wire::Error> {
        match self.rx.try_recv() {
            Ok(b) => wire::from_slice(&b).map(Some),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.closed = true;
                Ok(None)
            }
        }
    }

    fn closed(&self) -> bool {
        self.closed
    }
}
