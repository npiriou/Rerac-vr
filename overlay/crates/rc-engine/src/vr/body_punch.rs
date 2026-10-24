//! Physical punches of Clank and Giant Clank: a punch with either hand presses the original □ attack.
//! The game then plays its own punch (Clank's alternating arms, Giant Clank's left / right / both fists) with
//! its native hit spheres and damage, in front of the body, which faces where the player looks.
use bevy::prelude::*;

#[derive(Default)]
pub(super) struct BodyPunch {
    hands: [Hand; 2],
    square_until: f64,
}
#[derive(Default, Clone, Copy)]
struct Hand {
    previous: Option<(f64, Vec3)>,
    rest: Option<Vec3>,
    resting_since: Option<f64>,
    ready: bool,
    cooldown: f64,
}

impl BodyPunch {
    /// Feeds this frame's controller poses and head position (tracking space, metres). True while □ is held for a
    /// punch: long enough for a game tick to see the press, then released so the next punch is a new press.
    pub(super) fn press(&mut self, now: f64, hands: [Option<Transform>; 2], head: Option<Vec3>, active: bool) -> bool {
        if !active {
            *self = BodyPunch::default();
            return false;
        }
        for (side, pose) in hands.into_iter().enumerate() {
            let h = &mut self.hands[side];
            let Some(pose) = pose else {
                *h = Hand::default();
                continue;
            };
            let at = pose.translation;
            let Some((t, before)) = h.previous.replace((now, at)) else {
                h.rest = Some(at);
                continue;
            };
            let dt = (now - t) as f32;
            if !(0.002..0.1).contains(&dt) || at.distance(before) > 0.3 {
                // A tracking jump or a hitch is not a stroke.
                (h.rest, h.ready, h.resting_since) = (Some(at), false, None);
                continue;
            }
            let velocity = (at - before) / dt;
            let speed = velocity.length();
            // The Walloper's thresholds: a held fist first, then a fast extending stroke of a few centimetres.
            if speed < 0.55 {
                h.rest = Some(at);
                let since = *h.resting_since.get_or_insert(now);
                h.ready |= now - since >= 0.06;
                continue;
            }
            h.resting_since = None;
            let travelled = h.rest.map_or(0.0, |r| at.distance(r));
            let outward = head.map_or(Vec3::ZERO, |head| (at - head).normalize_or_zero());
            let extending = velocity.dot(*pose.forward()) > speed * 0.3 || velocity.dot(outward) > speed * 0.3;
            if h.ready && now >= h.cooldown && speed >= 1.45 && travelled >= 0.07 && extending {
                h.ready = false;
                h.cooldown = now + 0.3;
                self.square_until = now + 0.07;
                println!("VR_GAME body_punch side={side} speed={speed:.2}");
            }
        }
        now < self.square_until
    }
}
