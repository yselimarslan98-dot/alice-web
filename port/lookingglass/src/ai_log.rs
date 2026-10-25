//! Optional enemy AI trace (`?ai-log` in the browser, `--ai-log` natively) for diagnosing
//! pursuit and attack behaviour from real play. Each enemy is sampled on every phase change and
//! every half second; Alice's position every half second. The text is written to the user data
//! file `ai-log.txt` every 10 s and trimmed to the most recent ~4 MB.
use macroquad::prelude::*;
use std::{cell::RefCell, collections::HashMap, fmt::Write};

const FILE: &str = "ai-log.txt";
const SAMPLE: f32 = 0.5;
const FLUSH: f32 = 10.;
const LIMIT: usize = 4 << 20;

#[derive(Default)]
struct Log {
    text: String,
    clock: f32,
    alice_at: f32,
    flushed: f32,
    ids: HashMap<usize, u32>,
    last: HashMap<usize, (f32, String)>,
}
thread_local! {
    static LOG: RefCell<Option<Log>> = const { RefCell::new(None) };
}

pub fn enable() {
    LOG.with(|l| {
        let mut log = Log::default();
        log.text.push_str(
            "# alice-web enemy AI trace. t=seconds. A = Alice eye; E<id> = enemy: kind phase \
             feet d=distance-to-target vis=sees-target mem=memory hunt=leash-left\n",
        );
        *l.borrow_mut() = Some(log);
    });
}

fn with(f: impl FnOnce(&mut Log)) {
    LOG.with(|l| {
        if let Some(log) = l.borrow_mut().as_mut() {
            f(log);
        }
    });
}

/// Once per rendered frame with the simulated time step.
pub fn frame(dt: f32, alice: Vec3) {
    with(|log| {
        log.clock += dt.max(0.);
        if log.clock - log.alice_at >= SAMPLE {
            log.alice_at = log.clock;
            let _ = writeln!(log.text, "{:.2} A {:.0} {:.0} {:.0}", log.clock, alice.x, alice.y, alice.z);
        }
        if log.clock - log.flushed >= FLUSH {
            log.flushed = log.clock;
            flush(log);
        }
    });
}

pub fn map(name: &str) {
    with(|log| {
        log.ids.clear();
        log.last.clear();
        let _ = writeln!(log.text, "{:.2} MAP {name}", log.clock);
        flush(log);
    });
}

/// One enemy's state this step; `key` only needs to stay stable while the enemy lives.
#[allow(clippy::too_many_arguments)]
pub fn enemy(
    key: *const (),
    kind: &str,
    phase: impl std::fmt::Debug,
    feet: Vec3,
    distance: f32,
    visible: bool,
    memory: f32,
    hunt: f32,
) {
    with(|log| {
        let key = key as usize;
        let phase = format!("{phase:?}");
        let now = log.clock;
        // Unchanged enemies are resampled only while engaged; idle ones only on phase changes.
        let quiet = !visible && memory == 0. && hunt == 0.;
        if log
            .last
            .get(&key)
            .is_some_and(|(at, p)| *p == phase && (quiet || now - at < SAMPLE))
        {
            return;
        }
        let next = log.ids.len() as u32 + 1;
        let id = *log.ids.entry(key).or_insert(next);
        let _ = writeln!(
            log.text,
            "{now:.2} E{id} {kind} {phase} {:.0} {:.0} {:.0} d={distance:.0} vis={} mem={memory:.1} hunt={hunt:.1}",
            feet.x,
            feet.y,
            feet.z,
            u8::from(visible),
        );
        log.last.insert(key, (now, phase));
    });
}

fn flush(log: &mut Log) {
    if log.text.len() > LIMIT {
        let cut = log.text.len() - LIMIT / 2;
        let cut = log.text[cut..].find('\n').map_or(cut, |i| cut + i + 1);
        log.text.drain(..cut);
    }
    if let Err(e) = crate::userfs::write(std::path::Path::new(FILE), log.text.as_bytes()) {
        eprintln!("ai-log: {e}");
    }
}

#[cfg(test)]
pub fn text() -> String {
    LOG.with(|l| l.borrow().as_ref().map(|l| l.text.clone()).unwrap_or_default())
}
