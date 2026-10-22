//! OpenXR controllers feed the original libpad2 input, never a second locomotion/combat system.
use bevy::prelude::*;
use bevy_mod_openxr::{
    action_binding::{OxrSendActionBindings, OxrSuggestActionBinding},
    action_set_attaching::OxrAttachActionSet,
    action_set_syncing::{OxrActionSetSyncSet, OxrSyncActionSet},
    openxr_session_running,
    poll_events::{OxrEventHandlerExt, OxrEventIn},
    resources::OxrInstance,
    session::OxrSession,
};
use bevy_mod_xr::session::{session_available, XrPreDestroySession, XrSessionCreated};
use openxr::{Action, ActionSet, Path, Vector2f};
use rc_game::pad::{button, PadInput};

#[derive(Resource, Default)]
pub struct GamePad {
    pub pad: PadInput,
    pub active: bool,
    pub recenter: bool,
}
#[derive(Resource, Default)]
struct Focused(bool);
#[derive(Resource)]
struct Actions {
    set: ActionSet,
    movement: Action<Vector2f>,
    camera: Action<Vector2f>,
    fire: Action<f32>,
    strafe: Action<f32>,
    crouch: Action<f32>,
    buttons: Vec<(u32, Action<bool>)>,
    reset: Action<bool>,
}

pub struct GameInputPlugin;
impl Plugin for GameInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamePad>()
            .init_resource::<Focused>()
            .add_oxr_event_handler(focus_event)
            .add_systems(Startup, create.run_if(session_available))
            .add_systems(OxrSendActionBindings, bindings)
            .add_systems(XrSessionCreated, attach)
            .add_systems(XrPreDestroySession, clear)
            .add_systems(
                PreUpdate,
                sync.before(OxrActionSetSyncSet)
                    .run_if(openxr_session_running),
            )
            .add_systems(
                PreUpdate,
                sample
                    .after(OxrActionSetSyncSet)
                    .before(crate::input_map::sample_system),
            );
    }
}
fn create(instance: Res<OxrInstance>, mut commands: Commands) {
    let make = || -> anyhow::Result<Actions> {
        let set = instance.create_action_set("rerac_game", "ReRAC game controller", 0)?;
        let mut buttons = Vec::new();
        for (bit, name, label) in [
            (button::CROSS, "jump", "Jump / confirm"),
            (button::SQUARE, "wrench", "Wrench"),
            (button::TRIANGLE, "quick_select", "Quick select / back"),
            (button::SELECT, "map", "Map"),
            (button::START, "pause", "Pause menu"),
            (button::L3, "left_click", "Left stick click"),
            (
                button::R3,
                "right_click",
                "First person / right stick click",
            ),
        ] {
            buttons.push((bit, set.create_action(name, label, &[])?));
        }
        Ok(Actions {
            movement: set.create_action("move", "Move Ratchet", &[])?,
            camera: set.create_action("camera", "Original follow camera", &[])?,
            fire: set.create_action("fire", "Use equipped weapon", &[])?,
            strafe: set.create_action("strafe", "Strafe / aim", &[])?,
            crouch: set.create_action("crouch", "Crouch", &[])?,
            reset: set.create_action("recenter", "Recenter headset", &[])?,
            buttons,
            set,
        })
    };
    match make() {
        Ok(a) => {
            commands.insert_resource(a);
        }
        Err(e) => panic!("OpenXR game actions: {e:#}"),
    }
}
fn attach(a: Res<Actions>, mut out: MessageWriter<OxrAttachActionSet>) {
    out.write(OxrAttachActionSet(a.set.clone()));
}
fn sync(a: Res<Actions>, mut out: MessageWriter<OxrSyncActionSet>) {
    out.write(OxrSyncActionSet(a.set.clone()));
}
fn clear(mut pad: ResMut<GamePad>, mut focus: ResMut<Focused>) {
    *pad = GamePad::default();
    focus.0 = false;
}
fn focus_event(event: OxrEventIn, mut focus: ResMut<Focused>) {
    if let openxr::Event::SessionStateChanged(state) = *event {
        focus.0 = state.state() == openxr::SessionState::FOCUSED;
        println!("VR_GAME controller_focus={}", focus.0);
    }
}
fn bindings(a: Option<Res<Actions>>, mut out: MessageWriter<OxrSuggestActionBinding>) {
    let Some(a) = a else { return };
    let mut bind = |action, profile: &'static str, path: &'static str| {
        out.write(OxrSuggestActionBinding {
            action,
            interaction_profile: profile.into(),
            bindings: vec![path.into()],
        });
    };
    for profile in [
        "/interaction_profiles/oculus/touch_controller",
        "/interaction_profiles/valve/index_controller",
        "/interaction_profiles/microsoft/motion_controller",
    ] {
        bind(
            a.movement.as_raw(),
            profile,
            "/user/hand/left/input/thumbstick",
        );
        bind(
            a.camera.as_raw(),
            profile,
            "/user/hand/right/input/thumbstick",
        );
        bind(
            a.fire.as_raw(),
            profile,
            "/user/hand/right/input/trigger/value",
        );
        bind(
            a.strafe.as_raw(),
            profile,
            "/user/hand/left/input/trigger/value",
        );
        let index = profile.ends_with("index_controller");
        let microsoft = profile.ends_with("motion_controller");
        bind(
            a.crouch.as_raw(),
            profile,
            if index {
                "/user/hand/right/input/squeeze/force"
            } else if microsoft {
                "/user/hand/right/input/squeeze/click"
            } else {
                "/user/hand/right/input/squeeze/value"
            },
        );
        for (bit, action) in &a.buttons {
            let path = match *bit {
                button::CROSS => {
                    if microsoft {
                        "/user/hand/right/input/trackpad/click"
                    } else {
                        "/user/hand/right/input/a/click"
                    }
                }
                button::SQUARE => {
                    if microsoft {
                        "/user/hand/left/input/trackpad/click"
                    } else if index {
                        "/user/hand/left/input/a/click"
                    } else {
                        "/user/hand/left/input/x/click"
                    }
                }
                button::TRIANGLE => {
                    if microsoft {
                        "/user/hand/right/input/menu/click"
                    } else {
                        "/user/hand/right/input/b/click"
                    }
                }
                button::SELECT => {
                    if microsoft {
                        "/user/hand/left/input/thumbstick/click"
                    } else if index {
                        "/user/hand/left/input/b/click"
                    } else {
                        "/user/hand/left/input/y/click"
                    }
                }
                button::START => {
                    if index {
                        "/user/hand/left/input/b/click"
                    } else {
                        "/user/hand/left/input/menu/click"
                    }
                }
                button::L3 => "/user/hand/left/input/thumbstick/click",
                _ => "/user/hand/right/input/thumbstick/click",
            };
            // Index has no menu key: B pauses; the map remains on keyboard/gamepad.
            if index && *bit == button::SELECT {
                continue;
            }
            bind(action.as_raw(), profile, path);
        }
        bind(
            a.reset.as_raw(),
            profile,
            "/user/hand/left/input/thumbstick/click",
        );
    }
}

pub fn encode(movement: Vec2, camera: Vec2, buttons: u32) -> PadInput {
    // Match the original keyboard/gamepad adapter, including its round-gate compensation.
    let raw = |v: f32| {
        (127.5 + 127.5 * (v * crate::input_map::STICK_SCALE).clamp(-1.0, 1.0)).round() as u8
    };
    let mut p = PadInput::neutral().press(buttons);
    p.lx = raw(movement.x);
    p.ly = raw(-movement.y);
    p.rx = raw(camera.x);
    p.ry = raw(-camera.y);
    p
}
fn sample(
    a: Option<Res<Actions>>,
    session: Option<Res<OxrSession>>,
    focus: Res<Focused>,
    mut out: ResMut<GamePad>,
    mut previous: Local<u32>,
) {
    *out = GamePad::default();
    if !focus.0 {
        *previous = 0;
        return;
    }
    let (Some(a), Some(session)) = (a, session) else {
        return;
    };
    // OpenXR returns inactive action states while unfocused. Never retain the previous pressed input.
    let vector = |a: &Action<Vector2f>| {
        a.state(&session, Path::NULL)
            .ok()
            .filter(|s| s.is_active)
            .map(|s| Vec2::new(s.current_state.x, s.current_state.y))
    };
    let held = |a: &Action<bool>| {
        a.state(&session, Path::NULL)
            .ok()
            .is_some_and(|s| s.is_active && s.current_state)
    };
    let trigger = |a: &Action<f32>| {
        a.state(&session, Path::NULL)
            .ok()
            .is_some_and(|s| s.is_active && s.current_state > 0.55)
    };
    let movement = vector(&a.movement);
    let camera = vector(&a.camera);
    out.active = movement.is_some() || camera.is_some();
    let mut buttons = 0;
    for (bit, a) in &a.buttons {
        if held(a) {
            buttons |= *bit;
        }
    }
    if trigger(&a.fire) {
        buttons |= button::CIRCLE;
    }
    if trigger(&a.strafe) {
        buttons |= button::L1;
    }
    if trigger(&a.crouch) {
        buttons |= button::R1;
    }
    out.pad = encode(
        movement.unwrap_or_default(),
        camera.unwrap_or_default(),
        buttons,
    );
    out.recenter = held(&a.reset);
    if buttons != *previous {
        println!("VR_GAME controller_buttons={buttons:#06x} move={movement:?} camera={camera:?}");
        *previous = buttons;
    }
}

pub fn merge_pad(pad: &mut PadInput, xr: Option<&GamePad>) {
    let Some(xr) = xr.filter(|p| p.active) else {
        return;
    };
    if xr.pad.lx.abs_diff(128) > 1 || xr.pad.ly.abs_diff(128) > 1 {
        pad.lx = xr.pad.lx;
        pad.ly = xr.pad.ly;
    }
    if xr.pad.rx.abs_diff(128) > 1 || xr.pad.ry.abs_diff(128) > 1 {
        pad.rx = xr.pad.rx;
        pad.ry = xr.pad.ry;
    }
    *pad = pad.press(xr.pad.buttons as u32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_game::pad::PadState;
    #[test]
    fn controller_bytes_run_through_original_pad_decoder() {
        let p = encode(
            Vec2::new(0.71, 0.71),
            Vec2::ZERO,
            button::CROSS | button::CIRCLE | button::SQUARE | button::START,
        );
        let mut decoded = PadState::default();
        decoded.update(Some(&p.bytes()), false);
        assert!(decoded.lx.to_f32() > 0.8 && decoded.ly.to_f32() < -0.8);
        assert_eq!(decoded.raw & 0x8e0, 0x8e0);
        decoded.update(Some(&encode(Vec2::ZERO, Vec2::ZERO, 0).bytes()), false);
        assert_eq!(decoded.raw, 0);
        assert_eq!(decoded.rx.to_f32(), 0.0);
    }
    #[test]
    fn inactive_xr_preserves_gamepad_and_active_xr_merges_buttons() {
        let mut p = PadInput::neutral().stick(-1.0, 0.0).press(button::CROSS);
        let original = p;
        merge_pad(&mut p, Some(&GamePad::default()));
        assert_eq!(p, original);
        merge_pad(
            &mut p,
            Some(&GamePad {
                pad: encode(Vec2::Y, Vec2::ZERO, button::CIRCLE),
                active: true,
                recenter: false,
            }),
        );
        assert_eq!(p.ly, 0);
        assert_eq!(
            p.buttons as u32 & (button::CROSS | button::CIRCLE),
            button::CROSS | button::CIRCLE
        );
    }
}
