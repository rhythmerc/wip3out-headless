//! The game's sound in the room: its own mix as it plays it - the ship's engine, its wall hits, the game's effects -
//! rendered by the PlayStation's sound chip (the runtime's SPU) as the game steps, taken from the library
//! ([`crate::native::AudioTap`]) and handed to the runtime as a stream, played from the ship. In mono, its left and
//! right summed: the game pans its mix about its own camera behind the ship, which isn't where the player is; the
//! emitter at the ship places it about the player's head. The music (the disc's CD audio) is left out by the library
//! (`WO3_MUSIC=1`: in).
//!
//! The game makes its sound a frame (1/25 s) at a time, in a burst as it steps, and the runtime asks for a block every
//! hundredth of a second: what has come in is kept a little ahead ([`LEAD_FRAMES`]) so a step a little late isn't a
//! gap, and no more than [`MAX_FRAMES`] (older is dropped, so it never lags further behind the game).
use crate::native::AudioTap;
use engine_core::audio::Stream;
use std::collections::VecDeque;
use std::time::Duration;

/// The game's mix: 44.1 kHz (the SPU's own rate), stereo.
pub const RATE: u32 = 44_100;
/// A block: a hundredth of a second.
const BLOCK_FRAMES: usize = 441;
/// Kept in hand before playing (again, after running dry): a game frame and a half.
const LEAD_FRAMES: usize = 2_646;
/// Kept at most: three game frames.
const MAX_FRAMES: usize = 5_292;

/// The game's mix as the runtime's stream.
pub struct Mix {
    tap: AudioTap,
    take: Vec<i16>,
    /// Taken from the game, mono, not yet played.
    queue: VecDeque<f32>,
    /// Playing (else filling to its lead).
    playing: bool,
}

impl Mix {
    pub fn new(tap: AudioTap) -> Self {
        Self { tap, take: vec![0; MAX_FRAMES * 2], queue: VecDeque::with_capacity(MAX_FRAMES * 2), playing: false }
    }

    /// The block after `got` frames came in (left, right interleaved), or silence while held.
    fn next(&mut self, got: &[i16], paused: bool) -> Vec<f32> {
        let mut out = vec![0.0; BLOCK_FRAMES];
        if paused {
            // (held: what it had made is let go, not heard late)
            self.queue.clear();
            self.playing = false;
            return out;
        }
        self.queue.extend(got.chunks_exact(2).map(|lr| (f32::from(lr[0]) + f32::from(lr[1])) / 65536.0));
        if self.queue.len() > MAX_FRAMES {
            let over = self.queue.len() - MAX_FRAMES;
            self.queue.drain(..over);
        }
        if !self.playing && self.queue.len() >= LEAD_FRAMES {
            self.playing = true;
        }
        if self.playing {
            let n = BLOCK_FRAMES.min(self.queue.len());
            for (o, s) in out.iter_mut().zip(self.queue.drain(..n)) {
                *o = s;
            }
            if n < BLOCK_FRAMES {
                self.playing = false;
            }
        }
        out
    }
}

impl Stream for Mix {
    fn channels(&self) -> u16 {
        1
    }

    fn rate(&self) -> u32 {
        RATE
    }

    fn block(&self) -> Duration {
        Duration::from_secs_f64(BLOCK_FRAMES as f64 / f64::from(RATE))
    }

    fn render(&mut self, paused: bool) -> Result<Vec<f32>, String> {
        let mut take = std::mem::take(&mut self.take);
        let got = self.tap.take(&mut take);
        let out = self.next(&take[..got * 2], paused);
        self.take = take;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn none(_: *mut i16, _: u32) -> u32 {
        0
    }

    fn mix() -> Mix {
        Mix::new(crate::native::AudioTap::for_test(none))
    }

    #[test]
    fn it_waits_for_its_lead_then_plays_in_order() {
        let mut m = mix();
        let frame: Vec<i16> = (0..1764).flat_map(|i| [i as i16, i as i16]).collect();
        // (a game frame in: under the lead, silence)
        assert!(m.next(&frame, false).iter().all(|&s| s == 0.0));
        let out = m.next(&frame, false);
        assert_eq!(out[1], 2.0 / 65536.0);
        assert_eq!(out[440], 880.0 / 65536.0);
        assert_eq!(m.queue.len(), 1764 * 2 - BLOCK_FRAMES);
    }

    #[test]
    fn it_keeps_no_more_than_its_most_and_lets_go_when_held() {
        let mut m = mix();
        let lots = vec![1i16; MAX_FRAMES * 4];
        m.next(&lots, false);
        assert_eq!(m.queue.len(), MAX_FRAMES - BLOCK_FRAMES);
        assert!(m.next(&lots, true).iter().all(|&s| s == 0.0));
        assert!(m.queue.is_empty() && !m.playing);
    }

    #[test]
    fn running_dry_waits_for_the_lead_again() {
        let mut m = mix();
        m.next(&vec![1i16; LEAD_FRAMES * 2], false);
        while m.playing {
            m.next(&[], false);
        }
        assert!(m.next(&vec![1i16; 200], false).iter().all(|&s| s == 0.0));
    }
}
