//! The bridge's wire format: each game datagram sealed end to end between launchers.
//!
//! `ver:u8 (1), from:u8, counter:u64` big-endian, then the ChaCha20-Poly1305 ciphertext and its
//! 16-byte tag. The 10-byte header is the AAD, and the nonce is `from`, three zero bytes, then
//! `counter` big-endian, so members sharing the lobby key never reuse a nonce. A receiver keeps a
//! 64-counter sliding window per sender and rejects replays and counters older than it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use ring::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};

pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 10;
pub const TAG_LEN: usize = 16;
/// What sealing adds to a datagram.
pub const OVERHEAD: usize = HEADER_LEN + TAG_LEN;
/// How many counters behind the newest one a datagram may arrive and still be accepted.
pub const WINDOW: u64 = 64;

/// Why a sealed datagram was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reject {
    /// Shorter than a header and tag, or another wire version.
    Malformed,
    /// The header names a sender that isn't a remote member.
    UnknownMember,
    /// The header names a different member from the one the transport received it from.
    WrongSender,
    /// The tag doesn't verify: another key, or a changed header or ciphertext.
    BadTag,
    /// A counter already accepted from this sender.
    Replay,
    /// A counter older than the sender's window.
    TooOld,
}

pub fn header(from: u8, counter: u64) -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[0] = VERSION;
    h[1] = from;
    h[2..].copy_from_slice(&counter.to_be_bytes());
    h
}

pub fn nonce(from: u8, counter: u64) -> [u8; NONCE_LEN] {
    let mut n = [0u8; NONCE_LEN];
    n[0] = from;
    n[4..].copy_from_slice(&counter.to_be_bytes());
    n
}

fn key(bytes: &[u8; 32]) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, bytes).expect("a 32-byte key"))
}

/// Seals this member's outgoing datagrams, numbering them from one counter shared by every
/// destination.
pub struct Sealer {
    key: LessSafeKey,
    from: u8,
    counter: AtomicU64,
}

impl Sealer {
    pub fn new(key_bytes: &[u8; 32], from: u8, first_counter: u64) -> Self {
        Self {
            key: key(key_bytes),
            from,
            counter: AtomicU64::new(first_counter),
        }
    }

    /// Replace `out` with `plaintext` sealed under the next counter.
    pub fn seal(&self, plaintext: &[u8], out: &mut Vec<u8>) {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        out.clear();
        out.reserve(plaintext.len() + OVERHEAD);
        out.extend_from_slice(&header(self.from, counter));
        out.extend_from_slice(plaintext);
        let (head, body) = out.split_at_mut(HEADER_LEN);
        let nonce = Nonce::assume_unique_for_key(nonce(self.from, counter));
        let tag = self
            .key
            .seal_in_place_separate_tag(nonce, Aad::from(&*head), body)
            .expect("a datagram is far below the AEAD's length limit");
        out.extend_from_slice(tag.as_ref());
    }
}

/// Opens datagrams from the remote members, keeping a replay window for each.
pub struct Opener {
    key: LessSafeKey,
    windows: HashMap<u8, ReplayWindow>,
}

impl Opener {
    pub fn new(key_bytes: &[u8; 32], members: impl IntoIterator<Item = u8>) -> Self {
        Self {
            key: key(key_bytes),
            windows: members
                .into_iter()
                .map(|m| (m, ReplayWindow::default()))
                .collect(),
        }
    }

    /// Open `wire`, which the transport received from member `sender`, in place and return its
    /// plaintext.
    pub fn open<'a>(&mut self, sender: u8, wire: &'a mut [u8]) -> Result<&'a [u8], Reject> {
        if wire.len() < OVERHEAD || wire[0] != VERSION {
            return Err(Reject::Malformed);
        }
        let from = wire[1];
        let window = self.windows.get_mut(&from).ok_or(Reject::UnknownMember)?;
        if from != sender {
            return Err(Reject::WrongSender);
        }
        let counter = u64::from_be_bytes(wire[2..HEADER_LEN].try_into().expect("8 bytes"));
        window.check(counter)?;
        let (head, body) = wire.split_at_mut(HEADER_LEN);
        let nonce = Nonce::assume_unique_for_key(nonce(from, counter));
        let plain = self
            .key
            .open_in_place(nonce, Aad::from(&*head), body)
            .map_err(|_| Reject::BadTag)?;
        window.accept(counter);
        Ok(plain)
    }
}

/// The counters seen from one sender: the newest, and a bit for each of the [`WINDOW`] before
/// and including it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplayWindow {
    top: Option<u64>,
    seen: u64,
}

impl ReplayWindow {
    pub fn check(&self, counter: u64) -> Result<(), Reject> {
        let Some(top) = self.top else { return Ok(()) };
        if counter > top {
            return Ok(());
        }
        let age = top - counter;
        if age >= WINDOW {
            Err(Reject::TooOld)
        } else if self.seen & (1 << age) != 0 {
            Err(Reject::Replay)
        } else {
            Ok(())
        }
    }

    /// Record `counter`, which [`ReplayWindow::check`] passed and whose tag verified.
    pub fn accept(&mut self, counter: u64) {
        match self.top {
            Some(top) if counter <= top => self.seen |= 1 << (top - counter),
            Some(top) => {
                let shift = counter - top;
                self.seen = if shift >= WINDOW { 0 } else { self.seen << shift };
                self.seen |= 1;
                self.top = Some(counter);
            }
            None => {
                self.seen = 1;
                self.top = Some(counter);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7; 32];

    #[test]
    fn seals_and_opens_with_the_contract_layout() {
        let sealer = Sealer::new(&KEY, 3, 0x0102_0304_0506_0708);
        let mut wire = Vec::new();
        sealer.seal(b"hello game", &mut wire);
        assert_eq!(wire.len(), 10 + OVERHEAD);
        assert_eq!(wire[..HEADER_LEN], [1, 3, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_ne!(&wire[HEADER_LEN..HEADER_LEN + 10], b"hello game");
        let mut opener = Opener::new(&KEY, [3]);
        assert_eq!(opener.open(3, &mut wire).unwrap(), b"hello game");
    }

    #[test]
    fn counts_up_from_the_first_counter() {
        let sealer = Sealer::new(&KEY, 2, 41);
        let mut wire = Vec::new();
        sealer.seal(b"a", &mut wire);
        assert_eq!(wire[2..HEADER_LEN], 41u64.to_be_bytes());
        sealer.seal(b"b", &mut wire);
        assert_eq!(wire[2..HEADER_LEN], 42u64.to_be_bytes());
    }

    #[test]
    fn nonce_is_sender_three_zeros_and_the_counter() {
        assert_eq!(
            nonce(9, 0x1122_3344_5566_7788),
            [9, 0, 0, 0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
        );
        let sealer = Sealer::new(&KEY, 9, 0x1122_3344_5566_7788);
        let mut wire = Vec::new();
        sealer.seal(b"payload", &mut wire);
        let mut body = wire[HEADER_LEN..].to_vec();
        let nonce = Nonce::assume_unique_for_key(nonce(9, 0x1122_3344_5566_7788));
        let plain = key(&KEY)
            .open_in_place(nonce, Aad::from(&wire[..HEADER_LEN]), &mut body)
            .unwrap();
        assert_eq!(plain, b"payload");
    }

    #[test]
    fn the_header_is_bound_to_the_tag() {
        let sealer = Sealer::new(&KEY, 2, 100);
        let mut sealed = Vec::new();
        sealer.seal(b"bound", &mut sealed);
        for byte in 2..HEADER_LEN {
            let mut wire = sealed.clone();
            wire[byte] ^= 1;
            let mut opener = Opener::new(&KEY, [2]);
            assert_eq!(opener.open(2, &mut wire), Err(Reject::BadTag), "byte {byte}");
        }
        let mut wire = sealed.clone();
        wire[1] = 4;
        let mut opener = Opener::new(&KEY, [2, 4]);
        assert_eq!(opener.open(4, &mut wire), Err(Reject::BadTag));
        let mut wire = sealed.clone();
        *wire.last_mut().unwrap() ^= 0x80;
        assert_eq!(opener.open(2, &mut wire), Err(Reject::BadTag));
        let mut wire = sealed.clone();
        wire[HEADER_LEN] ^= 0x80;
        assert_eq!(opener.open(2, &mut wire), Err(Reject::BadTag));
    }

    #[test]
    fn rejects_malformed_unknown_and_misattributed_datagrams() {
        let sealer = Sealer::new(&KEY, 2, 0);
        let mut sealed = Vec::new();
        sealer.seal(b"x", &mut sealed);
        let mut opener = Opener::new(&KEY, [2, 3]);
        let mut short = sealed[..OVERHEAD - 1].to_vec();
        assert_eq!(opener.open(2, &mut short), Err(Reject::Malformed));
        let mut wire = sealed.clone();
        wire[0] = 2;
        assert_eq!(opener.open(2, &mut wire), Err(Reject::Malformed));
        let mut wire = sealed.clone();
        assert_eq!(opener.open(3, &mut wire), Err(Reject::WrongSender));
        let mut opener = Opener::new(&KEY, [3]);
        let mut wire = sealed.clone();
        assert_eq!(opener.open(2, &mut wire), Err(Reject::UnknownMember));
        let mut opener = Opener::new(&[8; 32], [2]);
        let mut wire = sealed.clone();
        assert_eq!(opener.open(2, &mut wire), Err(Reject::BadTag));
    }

    #[test]
    fn an_empty_datagram_round_trips() {
        let sealer = Sealer::new(&KEY, 5, 0);
        let mut wire = Vec::new();
        sealer.seal(b"", &mut wire);
        assert_eq!(wire.len(), OVERHEAD);
        assert_eq!(Opener::new(&KEY, [5]).open(5, &mut wire).unwrap(), b"");
    }

    fn feed(w: &mut ReplayWindow, counter: u64) -> Result<(), Reject> {
        w.check(counter)?;
        w.accept(counter);
        Ok(())
    }

    #[test]
    fn the_window_accepts_reordering_and_rejects_duplicates() {
        let mut w = ReplayWindow::default();
        assert_eq!(feed(&mut w, 10), Ok(()));
        assert_eq!(feed(&mut w, 12), Ok(()));
        assert_eq!(feed(&mut w, 11), Ok(()));
        assert_eq!(feed(&mut w, 11), Err(Reject::Replay));
        assert_eq!(feed(&mut w, 12), Err(Reject::Replay));
        assert_eq!(feed(&mut w, 10), Err(Reject::Replay));
        assert_eq!(feed(&mut w, 5), Ok(()));
        assert_eq!(feed(&mut w, 13), Ok(()));
    }

    #[test]
    fn the_window_edge_is_64_counters() {
        let mut w = ReplayWindow::default();
        feed(&mut w, 100).unwrap();
        assert_eq!(feed(&mut w, 100 - (WINDOW - 1)), Ok(()));
        assert_eq!(feed(&mut w, 100 - WINDOW), Err(Reject::TooOld));
        assert_eq!(feed(&mut w, 0), Err(Reject::TooOld));
        feed(&mut w, 100 + WINDOW).unwrap();
        assert_eq!(w.check(101), Ok(()));
        assert_eq!(w.check(100), Err(Reject::TooOld));
        assert_eq!(w.check(100 + WINDOW), Err(Reject::Replay));
    }

    #[test]
    fn the_window_slides_and_remembers_what_it_kept() {
        let mut w = ReplayWindow::default();
        feed(&mut w, 0).unwrap();
        assert_eq!(feed(&mut w, 0), Err(Reject::Replay));
        feed(&mut w, 3).unwrap();
        feed(&mut w, 40).unwrap();
        assert_eq!(w.check(3), Err(Reject::Replay));
        assert_eq!(w.check(2), Ok(()));
        feed(&mut w, 66).unwrap();
        assert_eq!(w.check(3), Err(Reject::Replay));
        assert_eq!(w.check(2), Err(Reject::TooOld));
        assert_eq!(w.check(40), Err(Reject::Replay));
        assert_eq!(w.check(41), Ok(()));
        feed(&mut w, u64::MAX).unwrap();
        assert_eq!(w.check(66), Err(Reject::TooOld));
        assert_eq!(w.check(u64::MAX), Err(Reject::Replay));
        assert_eq!(w.check(u64::MAX - 1), Ok(()));
    }

    #[test]
    fn a_failed_tag_leaves_the_window_alone() {
        let sealer = Sealer::new(&KEY, 2, 50);
        let mut good = Vec::new();
        sealer.seal(b"real", &mut good);
        let mut forged = good.clone();
        forged[2..HEADER_LEN].copy_from_slice(&1000u64.to_be_bytes());
        let mut opener = Opener::new(&KEY, [2]);
        assert_eq!(opener.open(2, &mut forged), Err(Reject::BadTag));
        let mut wire = good.clone();
        assert_eq!(opener.open(2, &mut wire).unwrap(), b"real");
        let mut wire = good.clone();
        assert_eq!(opener.open(2, &mut wire), Err(Reject::Replay));
    }
}
