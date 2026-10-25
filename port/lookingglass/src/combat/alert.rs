//! Short-lived alarm calls between enemies. A guard that spots or is struck by Alice raises an
//! alert; idle enemies nearby that hear it start pursuing the same point. Alerts are transient
//! (aged once per frame by the viewer) and never saved.
use super::*;
use std::cell::RefCell;

/// How long an alert stays audible.
const LIFE: f32 = 1.5;
/// Heard through open space up to this distance.
const RANGE: f32 = 900.;
/// Heard even around a corner up to this distance.
const NEAR: f32 = 360.;

struct Alert {
    from: Vec3,
    target: Vec3,
    age: f32,
}
thread_local! {
    static ALERTS: RefCell<Vec<Alert>> = const { RefCell::new(Vec::new()) };
}

pub fn raise(from: Vec3, target: Vec3) {
    if !from.is_finite() || !target.is_finite() {
        return;
    }
    ALERTS.with(|a| {
        let mut a = a.borrow_mut();
        if a.len() >= 32 {
            a.remove(0);
        }
        a.push(Alert { from, target, age: 0. });
    });
}

/// The pursuit point of an alert audible at `ear`, if any.
pub fn heard(world: &World, ear: Vec3) -> Option<Vec3> {
    ALERTS.with(|a| {
        a.borrow().iter().find_map(|alert| {
            let d = alert.from.distance(ear);
            if d < 1. || d > RANGE {
                return None;
            }
            let open = d <= NEAR || {
                let t = world.sweep(ear, alert.from, Vec3::splat(0.5));
                !t.start_solid && t.fraction >= 1.
            };
            open.then_some(alert.target)
        })
    })
}

pub fn age(dt: f32) {
    ALERTS.with(|a| {
        a.borrow_mut().retain_mut(|alert| {
            alert.age += dt.max(0.);
            alert.age < LIFE
        })
    });
}

pub fn clear() {
    ALERTS.with(|a| a.borrow_mut().clear());
}

/// How long an enemy keeps hunting around Alice's last known position after losing her.
pub const SEARCH: f32 = 8.;

/// Pursuit bookkeeping shared by ground enemies, layered over each one's own sight memory:
/// a fresh sighting or blow raises an alert, idle enemies answer nearby alerts, and once
/// memory runs out the enemy keeps searching (wandering around the last sighting) for a while.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Hunt {
    pub search: f32,
    wander: u32,
}
impl Hunt {
    pub fn valid(&self) -> bool {
        (0. ..=SEARCH).contains(&self.search)
    }
    /// Call before the enemy's own memory update for this step. `full` is its memory span.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        world: &World,
        ear: Vec3,
        aim: Vec3,
        visible: bool,
        notarget: bool,
        memory: &mut f32,
        last_seen: &mut Vec3,
        full: f32,
        dt: f32,
    ) {
        if notarget {
            self.search = 0.;
            return;
        }
        let hunting = self.search > 0.;
        if visible {
            if !hunting && *memory == 0. {
                raise(ear, aim);
            }
            self.search = SEARCH;
        } else if *memory > 0. && !hunting {
            // Struck from idle without a sighting: call for help toward the attacker.
            raise(ear, *last_seen);
            self.search = SEARCH;
        } else if !hunting {
            if let Some(p) = heard(world, ear) {
                *last_seen = p;
                *memory = full;
                self.search = SEARCH;
            }
        } else if *memory == 0. {
            self.search = (self.search - dt).max(0.);
        }
    }
    pub fn active(&self) -> bool {
        self.search > 0.
    }
    /// Pick the next point to check around the current position.
    pub fn wander(&mut self, feet: Vec3, last_seen: &mut Vec3) {
        if self.wander == 0 {
            self.wander = (feet.x.to_bits() ^ feet.y.to_bits().rotate_left(16)) | 1;
        }
        self.wander = self.wander.wrapping_mul(1664525).wrapping_add(1013904223);
        let a = (self.wander >> 8) as f32 / (1u32 << 24) as f32 * std::f32::consts::TAU;
        *last_seen = vec3(feet.x + a.cos() * 240., feet.y + a.sin() * 240., last_seen.z);
    }
    pub fn clear(&mut self) {
        self.search = 0.;
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alerts_reach_open_neighbours_not_sealed_rooms_and_expire() {
        clear();
        let world = World::fixture(&[
            (vec3(-2000., -500., -20.), vec3(2000., 500., 0.)),
            (vec3(-10., -500., 0.), vec3(10., 500., 300.)),
        ]);
        raise(vec3(-500., 0., 60.), vec3(-700., 0., 60.));
        assert_eq!(heard(&world, vec3(-900., 0., 60.)), Some(vec3(-700., 0., 60.)));
        assert_eq!(heard(&world, vec3(500., 0., 60.)), None, "wall blocks a distant call");
        assert_eq!(heard(&world, vec3(-1600., 0., 60.)), None, "out of range");
        age(LIFE);
        assert_eq!(heard(&world, vec3(-900., 0., 60.)), None, "expired");
    }
}
