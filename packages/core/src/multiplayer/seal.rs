//! The bridge's wire format: each game datagram sealed end to end between launchers.
//!
//! `ver:u8 (2), from:u8, session:u64, counter:u64` big-endian, then the ChaCha20-Poly1305
//! ciphertext and its 16-byte tag. A sender picks a random session id when its bridge starts and
//! seals under the subkey HKDF-SHA256(lobby key, empty salt, info = `swofty-netplay-v1` ‖
//! session ‖ from), counting from 0 for each destination. The AAD is the 18-byte header followed
//! by the receiver's lobby number, and the nonce is `from`, `to`, two zero bytes, then the
//! counter, so a datagram opens only at the member it was sealed for.
//!
//! A receiver keeps a [`WINDOW`]-counter sliding window for each sender's current session. A new
//! session from a sender replaces the old one once a datagram of it verifies, and the sessions it
//! replaced are rejected from then on.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use ring::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::hkdf;

pub const VERSION: u8 = 2;
pub const HEADER_LEN: usize = 18;
pub const TAG_LEN: usize = 16;
/// What sealing adds to a datagram.
pub const OVERHEAD: usize = HEADER_LEN + TAG_LEN;
/// How many counters behind the newest one a datagram may arrive and still be accepted.
pub const WINDOW: u64 = 1024;
const WORDS: usize = (WINDOW / 64) as usize;
const INFO: &[u8] = b"swofty-netplay-v1";

/// Why a sealed datagram was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reject {
    /// Shorter than a header and tag, or another wire version.
    Malformed,
    /// The header names a sender that isn't a remote member.
    UnknownMember,
    /// The header names a different member from the one the transport received it from.
    WrongSender,
    /// The tag doesn't verify: another key or recipient, or a changed header or ciphertext.
    BadTag,
    /// A counter already accepted in this session.
    Replay,
    /// A counter older than the session's window.
    TooOld,
    /// A session the sender has since replaced.
    OldSession,
}

pub fn header(from: u8, session: u64, counter: u64) -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[0] = VERSION;
    h[1] = from;
    h[2..10].copy_from_slice(&session.to_be_bytes());
    h[10..].copy_from_slice(&counter.to_be_bytes());
    h
}

pub fn nonce(from: u8, to: u8, counter: u64) -> [u8; NONCE_LEN] {
    let mut n = [0u8; NONCE_LEN];
    n[0] = from;
    n[1] = to;
    n[4..].copy_from_slice(&counter.to_be_bytes());
    n
}

/// The header followed by the receiver's lobby number.
pub fn aad(header: &[u8], to: u8) -> [u8; HEADER_LEN + 1] {
    let mut a = [0u8; HEADER_LEN + 1];
    a[..HEADER_LEN].copy_from_slice(&header[..HEADER_LEN]);
    a[HEADER_LEN] = to;
    a
}

/// The key member `from` seals with in `session`.
pub fn subkey(lobby_key: &[u8; 32], session: u64, from: u8) -> LessSafeKey {
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(lobby_key);
    let session = session.to_be_bytes();
    let info: [&[u8]; 3] = [INFO, &session, &[from]];
    let okm = prk
        .expand(&info, &CHACHA20_POLY1305)
        .expect("a 32-byte key is within HKDF's output limit");
    LessSafeKey::new(UnboundKey::from(okm))
}

/// Seals this member's outgoing datagrams in one session, with a counter per destination.
pub struct Sealer {
    key: LessSafeKey,
    from: u8,
    session: u64,
    counters: [AtomicU64; 256],
}

impl Sealer {
    pub fn new(lobby_key: &[u8; 32], from: u8, session: u64) -> Self {
        Self {
            key: subkey(lobby_key, session, from),
            from,
            session,
            counters: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    pub fn session(&self) -> u64 {
        self.session
    }

    /// Replace `out` with `plaintext` sealed for member `to` under its next counter.
    pub fn seal(&self, to: u8, plaintext: &[u8], out: &mut Vec<u8>) {
        let counter = self.counters[usize::from(to)].fetch_add(1, Ordering::Relaxed);
        self.seal_at(to, counter, plaintext, out);
    }

    fn seal_at(&self, to: u8, counter: u64, plaintext: &[u8], out: &mut Vec<u8>) {
        out.clear();
        out.reserve(plaintext.len() + OVERHEAD);
        out.extend_from_slice(&header(self.from, self.session, counter));
        out.extend_from_slice(plaintext);
        let aad = aad(out, to);
        let nonce = Nonce::assume_unique_for_key(nonce(self.from, to, counter));
        let tag = self
            .key
            .seal_in_place_separate_tag(nonce, Aad::from(aad), &mut out[HEADER_LEN..])
            .expect("a datagram is far below the AEAD's length limit");
        out.extend_from_slice(tag.as_ref());
    }
}

#[cfg(test)]
impl Sealer {
    /// Seal under a chosen counter, which the bridge never does.
    pub(crate) fn seal_with_counter(&self, to: u8, counter: u64, plaintext: &[u8], out: &mut Vec<u8>) {
        self.seal_at(to, counter, plaintext, out);
    }
}

/// Opens the datagrams sealed for member `me`, keeping each remote member's session.
pub struct Opener {
    lobby_key: [u8; 32],
    me: u8,
    peers: HashMap<u8, Peer>,
}

#[derive(Default)]
struct Peer {
    current: Option<Session>,
    retired: HashSet<u64>,
}

struct Session {
    id: u64,
    key: LessSafeKey,
    window: ReplayWindow,
}

impl Opener {
    pub fn new(lobby_key: &[u8; 32], me: u8, members: impl IntoIterator<Item = u8>) -> Self {
        Self {
            lobby_key: *lobby_key,
            me,
            peers: members.into_iter().map(|m| (m, Peer::default())).collect(),
        }
    }

    /// Open `wire`, which the transport received from member `sender`, in place and return its
    /// plaintext.
    pub fn open<'a>(&mut self, sender: u8, wire: &'a mut [u8]) -> Result<&'a [u8], Reject> {
        if wire.len() < OVERHEAD || wire[0] != VERSION {
            return Err(Reject::Malformed);
        }
        let from = wire[1];
        let peer = self.peers.get_mut(&from).ok_or(Reject::UnknownMember)?;
        if from != sender {
            return Err(Reject::WrongSender);
        }
        let session = u64::from_be_bytes(wire[2..10].try_into().expect("8 bytes"));
        let counter = u64::from_be_bytes(wire[10..HEADER_LEN].try_into().expect("8 bytes"));
        if peer.retired.contains(&session) {
            return Err(Reject::OldSession);
        }
        let aad = Aad::from(aad(wire, self.me));
        let nonce = Nonce::assume_unique_for_key(nonce(from, self.me, counter));
        let body = &mut wire[HEADER_LEN..];
        match &mut peer.current {
            Some(current) if current.id == session => {
                current.window.check(counter)?;
                let plain = current
                    .key
                    .open_in_place(nonce, aad, body)
                    .map_err(|_| Reject::BadTag)?;
                current.window.accept(counter);
                Ok(plain)
            }
            _ => {
                let key = subkey(&self.lobby_key, session, from);
                let plain = key.open_in_place(nonce, aad, body).map_err(|_| Reject::BadTag)?;
                let mut window = ReplayWindow::default();
                window.accept(counter);
                let next = Session {
                    id: session,
                    key,
                    window,
                };
                if let Some(old) = peer.current.replace(next) {
                    peer.retired.insert(old.id);
                }
                Ok(plain)
            }
        }
    }
}

/// The counters seen in one session: the newest, and a bit for each of the [`WINDOW`] up to and
/// including it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplayWindow {
    top: Option<u64>,
    seen: [u64; WORDS],
}

impl ReplayWindow {
    pub fn check(&self, counter: u64) -> Result<(), Reject> {
        let Some(top) = self.top else { return Ok(()) };
        if counter > top {
            Ok(())
        } else if top - counter >= WINDOW {
            Err(Reject::TooOld)
        } else if self.is_set(counter) {
            Err(Reject::Replay)
        } else {
            Ok(())
        }
    }

    /// Record `counter`, which [`ReplayWindow::check`] passed and whose tag verified.
    pub fn accept(&mut self, counter: u64) {
        match self.top {
            Some(top) if counter <= top => {}
            Some(top) if counter - top < WINDOW => {
                for n in top + 1..counter {
                    self.clear(n);
                }
                self.top = Some(counter);
            }
            _ => {
                self.seen = [0; WORDS];
                self.top = Some(counter);
            }
        }
        let (word, bit) = Self::slot(counter);
        self.seen[word] |= bit;
    }

    fn slot(counter: u64) -> (usize, u64) {
        let index = counter % WINDOW;
        ((index / 64) as usize, 1 << (index % 64))
    }

    fn is_set(&self, counter: u64) -> bool {
        let (word, bit) = Self::slot(counter);
        self.seen[word] & bit != 0
    }

    fn clear(&mut self, counter: u64) {
        let (word, bit) = Self::slot(counter);
        self.seen[word] &= !bit;
    }
}

#[cfg(test)]
mod tests {
    use ring::hmac;

    use super::*;

    const KEY: [u8; 32] = [7; 32];
    const SESSION: u64 = 0x0102_0304_0506_0708;

    fn sealed(from: u8, to: u8, plaintext: &[u8]) -> Vec<u8> {
        let mut wire = Vec::new();
        Sealer::new(&KEY, from, SESSION).seal(to, plaintext, &mut wire);
        wire
    }

    #[test]
    fn seals_and_opens_with_the_contract_layout() {
        let sealer = Sealer::new(&KEY, 3, SESSION);
        let mut wire = Vec::new();
        sealer.seal_with_counter(1, 0x1112_1314_1516_1718, b"hello game", &mut wire);
        assert_eq!(wire.len(), 10 + OVERHEAD);
        assert_eq!(
            wire[..HEADER_LEN],
            [
                2, 3, 1, 2, 3, 4, 5, 6, 7, 8, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18
            ]
        );
        assert_ne!(&wire[HEADER_LEN..HEADER_LEN + 10], b"hello game");
        let mut opener = Opener::new(&KEY, 1, [3]);
        assert_eq!(opener.open(3, &mut wire).unwrap(), b"hello game");
    }

    #[test]
    fn counts_from_zero_for_each_destination() {
        let sealer = Sealer::new(&KEY, 2, SESSION);
        let mut wire = Vec::new();
        let counter = |wire: &[u8]| u64::from_be_bytes(wire[10..HEADER_LEN].try_into().unwrap());
        sealer.seal(1, b"a", &mut wire);
        assert_eq!(counter(&wire), 0);
        sealer.seal(3, b"b", &mut wire);
        assert_eq!(counter(&wire), 0);
        sealer.seal(1, b"c", &mut wire);
        assert_eq!(counter(&wire), 1);
        assert_eq!(wire[2..10], SESSION.to_be_bytes());
    }

    #[test]
    fn the_subkey_is_hkdf_sha256_over_the_session_and_sender() {
        let prk = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &[0; 32]), &KEY);
        let mut info = b"swofty-netplay-v1".to_vec();
        info.extend_from_slice(&SESSION.to_be_bytes());
        info.extend_from_slice(&[9, 1]);
        let okm = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, prk.as_ref()), &info);
        let expected = LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, okm.as_ref()).unwrap());

        let mut wire = Vec::new();
        Sealer::new(&KEY, 9, SESSION).seal_with_counter(4, 77, b"payload", &mut wire);
        assert_eq!(nonce(9, 4, 77), [9, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77]);
        let mut body = wire[HEADER_LEN..].to_vec();
        let nonce = Nonce::assume_unique_for_key(nonce(9, 4, 77));
        let mut ad = wire[..HEADER_LEN].to_vec();
        ad.push(4);
        let plain = expected.open_in_place(nonce, Aad::from(&ad), &mut body).unwrap();
        assert_eq!(plain, b"payload");
    }

    #[test]
    fn a_datagram_opens_only_at_its_recipient() {
        let mut wire = sealed(1, 3, b"for three");
        let mut at_two = Opener::new(&KEY, 2, [1, 3]);
        assert_eq!(at_two.open(1, &mut wire.clone()), Err(Reject::BadTag));
        let mut at_three = Opener::new(&KEY, 3, [1, 2]);
        assert_eq!(at_three.open(1, &mut wire).unwrap(), b"for three");
    }

    #[test]
    fn the_header_is_bound_to_the_tag() {
        let sealed = sealed(2, 1, b"bound");
        for byte in 2..HEADER_LEN {
            let mut wire = sealed.clone();
            wire[byte] ^= 1;
            let mut opener = Opener::new(&KEY, 1, [2]);
            assert_eq!(opener.open(2, &mut wire), Err(Reject::BadTag), "byte {byte}");
        }
        let mut wire = sealed.clone();
        wire[1] = 4;
        let mut opener = Opener::new(&KEY, 1, [2, 4]);
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
        let sealed = sealed(2, 1, b"x");
        let mut opener = Opener::new(&KEY, 1, [2, 3]);
        let mut short = sealed[..OVERHEAD - 1].to_vec();
        assert_eq!(opener.open(2, &mut short), Err(Reject::Malformed));
        let mut wire = sealed.clone();
        wire[0] = 1;
        assert_eq!(opener.open(2, &mut wire), Err(Reject::Malformed));
        assert_eq!(opener.open(3, &mut sealed.clone()), Err(Reject::WrongSender));
        let mut opener = Opener::new(&KEY, 1, [3]);
        assert_eq!(opener.open(2, &mut sealed.clone()), Err(Reject::UnknownMember));
        let mut opener = Opener::new(&[8; 32], 1, [2]);
        assert_eq!(opener.open(2, &mut sealed.clone()), Err(Reject::BadTag));
    }

    #[test]
    fn an_empty_datagram_round_trips() {
        let mut wire = sealed(5, 1, b"");
        assert_eq!(wire.len(), OVERHEAD);
        assert_eq!(Opener::new(&KEY, 1, [5]).open(5, &mut wire).unwrap(), b"");
    }

    #[test]
    fn a_restarted_sender_never_reuses_a_keystream() {
        let (mut a, mut b) = (Vec::new(), Vec::new());
        Sealer::new(&KEY, 2, 1).seal(1, b"same plaintext", &mut a);
        Sealer::new(&KEY, 2, 2).seal(1, b"same plaintext", &mut b);
        assert_eq!(a[10..HEADER_LEN], b[10..HEADER_LEN]);
        assert_ne!(a[HEADER_LEN..], b[HEADER_LEN..]);
    }

    #[test]
    fn a_new_session_replaces_the_old_one_for_good() {
        let (old, new) = (Sealer::new(&KEY, 2, 10), Sealer::new(&KEY, 2, 11));
        let mut opener = Opener::new(&KEY, 1, [2]);
        let (mut old_wires, mut wire) = (Vec::new(), Vec::new());
        for _ in 0..3 {
            old.seal(1, b"old", &mut wire);
            old_wires.push(wire.clone());
        }
        assert_eq!(opener.open(2, &mut old_wires[0].clone()).unwrap(), b"old");
        assert_eq!(opener.open(2, &mut old_wires[1].clone()).unwrap(), b"old");
        new.seal(1, b"new", &mut wire);
        assert_eq!(opener.open(2, &mut wire.clone()).unwrap(), b"new");
        assert_eq!(opener.open(2, &mut wire), Err(Reject::Replay));
        assert_eq!(opener.open(2, &mut old_wires[2].clone()), Err(Reject::OldSession));
        assert_eq!(opener.open(2, &mut old_wires[0].clone()), Err(Reject::OldSession));
        new.seal(1, b"next", &mut wire);
        assert_eq!(opener.open(2, &mut wire).unwrap(), b"next");
    }

    #[test]
    fn a_forged_new_session_leaves_the_current_one_alone() {
        let real = Sealer::new(&KEY, 2, 10);
        let mut opener = Opener::new(&KEY, 1, [2]);
        let mut wire = Vec::new();
        real.seal(1, b"first", &mut wire);
        opener.open(2, &mut wire).unwrap();
        let mut forged = Vec::new();
        Sealer::new(&[9; 32], 2, 99).seal(1, b"forged", &mut forged);
        assert_eq!(opener.open(2, &mut forged), Err(Reject::BadTag));
        real.seal(1, b"second", &mut wire);
        assert_eq!(opener.open(2, &mut wire).unwrap(), b"second");
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
    fn the_window_edge_is_1024_counters() {
        let mut w = ReplayWindow::default();
        feed(&mut w, 5000).unwrap();
        assert_eq!(feed(&mut w, 5000 - (WINDOW - 1)), Ok(()));
        assert_eq!(feed(&mut w, 5000 - WINDOW), Err(Reject::TooOld));
        assert_eq!(feed(&mut w, 0), Err(Reject::TooOld));
        feed(&mut w, 5000 + WINDOW).unwrap();
        assert_eq!(w.check(5001), Ok(()));
        assert_eq!(w.check(5000), Err(Reject::TooOld));
        assert_eq!(w.check(5000 + WINDOW), Err(Reject::Replay));
    }

    #[test]
    fn the_window_slides_and_remembers_what_it_kept() {
        let mut w = ReplayWindow::default();
        feed(&mut w, 0).unwrap();
        assert_eq!(feed(&mut w, 0), Err(Reject::Replay));
        feed(&mut w, 3).unwrap();
        feed(&mut w, 700).unwrap();
        assert_eq!(w.check(3), Err(Reject::Replay));
        assert_eq!(w.check(2), Ok(()));
        feed(&mut w, 1026).unwrap();
        assert_eq!(w.check(3), Err(Reject::Replay));
        assert_eq!(w.check(2), Err(Reject::TooOld));
        assert_eq!(w.check(700), Err(Reject::Replay));
        assert_eq!(w.check(701), Ok(()));
        assert_eq!(w.check(1026), Err(Reject::Replay));
        assert_eq!(w.check(1025), Ok(()));
        feed(&mut w, u64::MAX).unwrap();
        assert_eq!(w.check(1026), Err(Reject::TooOld));
        assert_eq!(w.check(u64::MAX), Err(Reject::Replay));
        assert_eq!(w.check(u64::MAX - 1), Ok(()));
    }

    #[test]
    fn a_datagram_overtaken_by_a_big_lobby_still_opens() {
        let sealer = Sealer::new(&KEY, 2, SESSION);
        let mut opener = Opener::new(&KEY, 1, [2]);
        let (mut late, mut wire) = (Vec::new(), Vec::new());
        sealer.seal(1, b"late", &mut late);
        for _ in 0..1000 {
            sealer.seal(1, b"on time", &mut wire);
            opener.open(2, &mut wire).unwrap();
        }
        assert_eq!(opener.open(2, &mut late).unwrap(), b"late");
    }

    #[test]
    fn a_failed_tag_leaves_the_window_alone() {
        let sealer = Sealer::new(&KEY, 2, SESSION);
        let mut good = Vec::new();
        sealer.seal_with_counter(1, 50, b"real", &mut good);
        let mut opener = Opener::new(&KEY, 1, [2]);
        assert_eq!(opener.open(2, &mut good.clone()).unwrap(), b"real");
        let mut forged = good.clone();
        forged[10..HEADER_LEN].copy_from_slice(&5000u64.to_be_bytes());
        assert_eq!(opener.open(2, &mut forged), Err(Reject::BadTag));
        let mut next = Vec::new();
        sealer.seal_with_counter(1, 49, b"older", &mut next);
        assert_eq!(opener.open(2, &mut next).unwrap(), b"older");
        assert_eq!(opener.open(2, &mut good), Err(Reject::Replay));
    }
}
