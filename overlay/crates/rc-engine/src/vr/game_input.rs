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
    spaces::OxrSpaceLocationFlags,
};
use bevy_mod_xr::{
    session::{XrPreDestroySession, XrSessionCreated, session_available},
    spaces::XrSpaceSyncSet,
};
use openxr::{Action, ActionSet, Haptic, Path, Posef, Vector2f};
use rc_game::pad::{PadInput, button};

#[derive(Resource, Default)]
pub struct GamePad {
    pub pad: PadInput,
    pub active: bool,
    pub quick_select: Option<Vec2>,
    pub vendor: Option<PadInput>,
    pub skip_cutscene: bool,
    /// Akimbo (super::akimbo): the left hand's ring is open (the left stick's click held), with where the left hand
    /// points in it; the left trigger fires the left hand's weapon.
    pub left_select: Option<Vec2>,
    pub left_fire: bool,
}
#[derive(Resource, Default)]
pub(super) struct Focused(pub bool);
#[derive(Resource, Default)]
pub(super) struct HandInput {
    pub grip: Option<f32>,
    pub left_grip: Option<f32>,
    pub trigger: Option<f32>,
    firing: bool,
    pack_modifier: bool,
    /// The left trigger past its threshold (the pack modifier's hysteresis): the left hand's weapon fires (akimbo).
    left_firing: bool,
    pub pack_jump: bool,
    pub menu_click: bool,
    pub menu_held: bool,
    /// The open menu page has pointer targets (super::menu_pointer, the last tick): pointing selects there, so the
    /// sticks do not navigate it, and A clicks like the trigger.
    pub menu_targets: bool,
    /// A is held (its press confirms in menus like the trigger's).
    menu_cross: bool,
    pub utility_cancel: bool,
    menu_ready: bool,
    menu_active: bool,
    menu_consumed: bool,
    quick_page: i8,
    /// The left trigger pressed with the left hand's ring open: its next page (taken by super::akimbo's tick).
    pub left_page: bool,
}
#[derive(Component, Clone, Copy)]
pub(super) struct Hand(pub usize);

#[derive(Resource, Default)]
pub(super) struct Feedback {
    pub wrench_impact: bool,
    pub wrench_wall: Option<f32>,
}

#[derive(Default)]
pub(super) struct HandSelect {
    held: bool,
    anchor: Option<(Vec3, Quat)>,
    previous: Option<Vec3>,
    revision: u64,
    selecting: bool,
    /// The trigger is held since it last turned the page (or since the ring opened).
    page_held: bool,
}
impl HandSelect {
    fn direction(&mut self, hand: Option<Vec3>, head: Option<Transform>, revision: u64) -> Vec2 {
        if self.revision != revision {
            *self = Self {
                held: true,
                revision,
                page_held: self.page_held,
                ..default()
            };
        }
        let (Some(hand), Some(head)) = (hand, head) else {
            self.anchor = None;
            self.previous = None;
            self.selecting = false;
            return Vec2::ZERO;
        };
        if self
            .previous
            .replace(hand)
            .is_some_and(|p| p.distance(hand) > 0.5)
        {
            self.anchor = None;
            self.selecting = false;
        }
        // Fixed tracking-space origin and view plane at Triangle press: head/rig motion cannot move the choice.
        let (origin, orientation) = *self.anchor.get_or_insert((hand, head.rotation));
        let delta = orientation.inverse() * (hand - origin);
        let direction = Vec2::new(delta.x, delta.y);
        let distance = direction.length();
        if distance > 0.06 {
            self.selecting = true;
        } else if distance < 0.035 {
            self.selecting = false;
        }
        if self.selecting {
            direction.normalize_or_zero()
        } else {
            Vec2::ZERO
        }
    }
}
/// Akimbo: the left hand's ring pick, and a left trigger already used (the ring was open, a menu) that fires nothing
/// until released.
#[derive(Default)]
pub(super) struct LeftHand {
    select: HandSelect,
    consumed: bool,
    /// The left grip's squeeze last sample, and whether it took the right weapon's foregrip (no R1 then).
    squeezed: bool,
    support: bool,
    /// The left trigger is held since it last turned the left ring's page.
    page_held: bool,
}

#[derive(Default)]
pub(super) struct SkipInput {
    media: Option<rc_game::menus::mode::Mode>,
    down: bool,
    pending: bool,
    suppress: bool,
}

#[derive(Default)]
pub(super) struct JumpInput {
    pressed: bool,
    trigger_consumed: bool,
    shopping: bool,
    /// The hero was the Hologuise disguise last frame, and the right hand's salute gesture.
    disguised: bool,
    salute: Salute,
}
/// The right hand's salute (the Hologuise's ○): the right hand raised open, at shoulder height or above, within 0.75 of
/// the head and not across to the left (a salute at the brow, or a wave), held 5 frames; it lets go with some slack.
#[derive(Default)]
pub(super) struct Salute {
    frames: u8,
    active: bool,
}
impl Salute {
    /// `head` and `hand` in tracking space. Head-local metres: +x right, +y up, −z forward.
    fn update(&mut self, head: Option<Transform>, hand: Option<Transform>, grip: f32) -> bool {
        let (Some(head), Some(hand)) = (head, hand) else {
            *self = Salute::default();
            return false;
        };
        let inv = head.rotation.inverse();
        let p = inv * (hand.translation - head.translation);
        // Any raised open right hand: at the brow, or up and waving.
        let pose = |m: f32| grip < 0.6 + m && p.y > -0.15 - m && p.x > -0.15 - m && p.length() < 0.75 + m;
        if self.active {
            self.active = pose(0.1);
        } else if pose(0.0) {
            self.frames = self.frames.saturating_add(1);
            self.active = self.frames >= 5;
            if self.active {
                println!("VR_GAME salute at={p:?}");
            }
        } else {
            self.frames = 0;
        }
        self.active
    }
}

#[derive(Resource)]
pub(super) struct Actions {
    set: ActionSet,
    movement: Action<Vector2f>,
    camera: Action<Vector2f>,
    grip: Action<f32>,
    left_grip: Action<f32>,
    left_trigger: Action<f32>,
    trigger: Action<f32>,
    pub(super) vibration: [Action<Haptic>; 2],
    hands: [Action<Posef>; 2],
    buttons: Vec<(u32, Action<bool>)>,
}

pub struct GameInputPlugin;
impl Plugin for GameInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamePad>()
            .init_resource::<Focused>()
            .init_resource::<HandInput>()
            .add_oxr_event_handler(focus_event)
            .add_systems(Startup, create.run_if(session_available))
            .add_systems(OxrSendActionBindings, bindings)
            .add_systems(XrSessionCreated, (attach, spawn_hands))
            .add_systems(XrPreDestroySession, clear)
            .init_resource::<super::haptics::State>()
            .add_systems(
                FixedUpdate,
                super::haptics::update
                    .after(crate::gameplay::GameTick)
                    .after(crate::menu_render::MenuFrameSet),
            )
            .add_systems(
                FixedUpdate,
                hand_select_menu
                    .after(crate::gameplay::GameTick)
                    .before(crate::menu_render::MenuFrameSet),
            )
            .add_systems(FixedUpdate, keep_stick_with_hero.after(crate::menu_render::MenuFrameSet))
            .add_systems(
                PreUpdate,
                sync.before(OxrActionSetSyncSet)
                    .run_if(openxr_session_running),
            )
            .add_systems(
                PreUpdate,
                sample
                    .after(OxrActionSetSyncSet)
                    .after(XrSpaceSyncSet)
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
            (button::CIRCLE, "weapon_button", "Weapon / circle"),
            (
                button::TRIANGLE,
                "quick_select",
                "Quick select / interact / back",
            ),
            (button::SELECT, "map", "Map"),
            (button::START, "pause", "Pause / skip cinematic"),
        ] {
            buttons.push((bit, set.create_action(name, label, &[])?));
        }
        Ok(Actions {
            movement: set.create_action("move", "Move Ratchet", &[])?,
            camera: set.create_action("camera", "Snap turn", &[])?,
            grip: set.create_action("grip", "Hold wrench / release to throw", &[])?,
            left_grip: set.create_action("left_grip", "R1 / support barrel / grab ledge", &[])?,
            left_trigger: set.create_action(
                "pack_modifier",
                "Left-hand weapon (Akimbo) or R1 / hoverboard boost",
                &[],
            )?,
            trigger: set.create_action("bomb_trigger", "Hold bomb / release to throw", &[])?,
            vibration: [
                set.create_action("left_feedback", "Left-hand vibration", &[])?,
                set.create_action("right_feedback", "Right-hand vibration", &[])?,
            ],
            hands: [
                set.create_action("left_hand", "Left hand pose", &[])?,
                set.create_action("right_hand", "Right hand pose", &[])?,
            ],
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
fn clear(
    mut commands: Commands,
    hands: Query<Entity, With<Hand>>,
    mut input: ResMut<HandInput>,
    mut pad: ResMut<GamePad>,
    mut focus: ResMut<Focused>,
) {
    for e in &hands {
        commands.entity(e).despawn();
    }
    *input = HandInput::default();
    *pad = GamePad::default();
    focus.0 = false;
}
fn spawn_hands(a: Res<Actions>, session: Res<OxrSession>, mut commands: Commands) {
    for (side, action) in a.hands.iter().enumerate() {
        match session.create_action_space(action, Path::NULL, Isometry3d::IDENTITY) {
            Ok(space) => {
                commands.spawn((
                    Hand(side),
                    space,
                    crate::level_switch::KeepAcrossLevels,
                    Name::new(format!("Ratchet tracked hand {side}")),
                ));
            }
            Err(e) => error!("VR_GAME cannot create hand tracking space: {e}"),
        }
    }
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
            a.hands[0].as_raw(),
            profile,
            "/user/hand/left/input/grip/pose",
        );
        bind(
            a.hands[1].as_raw(),
            profile,
            "/user/hand/right/input/grip/pose",
        );
        bind(
            a.trigger.as_raw(),
            profile,
            "/user/hand/right/input/trigger/value",
        );
        bind(
            a.vibration[0].as_raw(),
            profile,
            "/user/hand/left/output/haptic",
        );
        bind(
            a.vibration[1].as_raw(),
            profile,
            "/user/hand/right/output/haptic",
        );
        bind(
            a.left_trigger.as_raw(),
            profile,
            "/user/hand/left/input/trigger/value",
        );
        let index = profile.ends_with("index_controller");
        let microsoft = profile.ends_with("motion_controller");
        bind(
            a.grip.as_raw(),
            profile,
            if index {
                "/user/hand/right/input/squeeze/force"
            } else if microsoft {
                "/user/hand/right/input/squeeze/click"
            } else {
                "/user/hand/right/input/squeeze/value"
            },
        );
        bind(
            a.left_grip.as_raw(),
            profile,
            if index {
                "/user/hand/left/input/squeeze/force"
            } else if microsoft {
                "/user/hand/left/input/squeeze/click"
            } else {
                "/user/hand/left/input/squeeze/value"
            },
        );
        for (bit, action) in &a.buttons {
            if let Some(path) = button_path(*bit, profile) {
                bind(action.as_raw(), profile, path);
            }
        }
    }
}

// Touch/PICO: A/B on the right, X/Y on the left. Index uses left A/B for X/Y.
// Y is Start/pause/skip; the right stick click is Triangle (interaction/quick select).
fn button_path(bit: u32, profile: &str) -> Option<&'static str> {
    let index = profile.ends_with("index_controller");
    let microsoft = profile.ends_with("motion_controller");
    Some(match bit {
        button::CROSS if microsoft => "/user/hand/right/input/trackpad/click",
        button::CROSS => "/user/hand/right/input/a/click",
        button::SQUARE if microsoft => "/user/hand/right/input/menu/click",
        button::SQUARE => "/user/hand/right/input/b/click",
        button::CIRCLE if microsoft => "/user/hand/left/input/trackpad/click",
        button::CIRCLE if index => "/user/hand/left/input/a/click",
        button::CIRCLE => "/user/hand/left/input/x/click",
        button::TRIANGLE => "/user/hand/right/input/thumbstick/click",
        button::SELECT => "/user/hand/left/input/thumbstick/click",
        button::START if microsoft => "/user/hand/left/input/menu/click",
        button::START if index => "/user/hand/left/input/b/click",
        button::START => "/user/hand/left/input/y/click",
        _ => return None,
    })
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

/// On the hoverboard, the right stick pushed past [`BOARD_TRICK_PUSH`] holds one of the four trick shoulders: up L1,
/// right R1, down L2, left R2 (they only count in a jump).
const BOARD_TRICK_PUSH: f32 = 0.6;
fn board_tricks(stick: Vec2) -> u32 {
    if stick.length() < BOARD_TRICK_PUSH {
        0
    } else if stick.y.abs() >= stick.x.abs() {
        if stick.y > 0.0 { button::L1 } else { button::L2 }
    } else if stick.x > 0.0 {
        button::R1
    } else {
        button::R2
    }
}
fn menu_directions(left: Vec2, right: Vec2) -> u32 {
    // The ship list sets lf::BUTTONS_ONLY: stick-derived PadState::held bits never reach its
    // raw_pressed input. Send actual D-pad buttons, so either XR stick can navigate the menus.
    let stick = if left.length_squared() >= right.length_squared() {
        left
    } else {
        right
    };
    if stick.abs().max_element() < 0.55 {
        return 0;
    }
    if stick.x.abs() > stick.y.abs() {
        if stick.x > 0.0 {
            button::RIGHT
        } else {
            button::LEFT
        }
    } else if stick.y > 0.0 {
        button::UP
    } else {
        button::DOWN
    }
}

pub(super) fn sample(
    a: Option<Res<Actions>>,
    session: Option<Res<OxrSession>>,
    focus: Res<Focused>,
    mut rig: ResMut<super::game::Rig>,
    play: Option<Res<crate::gameplay::Play>>,
    mut hand: ResMut<HandInput>,
    views: Option<Res<bevy_mod_openxr::resources::OxrViews>>,
    mode: Res<crate::menu_render::MenuMode>,
    mut out: ResMut<GamePad>,
    mut previous: Local<u32>,
    tracked: Query<(&Hand, &Transform, &OxrSpaceLocationFlags)>,
    mut selection: Local<HandSelect>,
    mut jump: Local<JumpInput>,
    mut skip: Local<SkipInput>,
    (mut utility_down, mut utility_consumed, mut left_squeeze): (Local<u32>, Local<bool>, Local<bool>),
    (support, akimbo, mut left, options, time): (
        Option<Res<super::blaster::SupportGrip>>,
        Option<Res<super::akimbo::Akimbo>>,
        Local<LeftHand>,
        Option<Res<crate::render_settings::VrOptions>>,
        Res<Time>,
    ),
) {
    *out = GamePad::default();
    hand.grip = None;
    hand.left_grip = None;
    hand.trigger = None;
    if !focus.0 {
        rig.hold_snap(1.0);
        hand.firing = false;
        hand.menu_click = false;
        hand.menu_held = false;
        hand.menu_cross = false;
        hand.menu_ready = false;
        hand.menu_active = false;
        hand.menu_consumed = true;
        hand.pack_modifier = false;
        hand.left_firing = false;
        *left = LeftHand::default();
        hand.pack_jump = false;
        hand.quick_page = 0;
        *jump = JumpInput::default();
        *skip = SkipInput::default();
        *previous = 0;
        *selection = HandSelect::default();
        hand.utility_cancel = false;
        *utility_down = 0;
        *utility_consumed = false;
        *left_squeeze = false;
        return;
    }
    let (Some(a), Some(session)) = (a, session) else {
        rig.hold_snap(1.0);
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
    hand.grip = a
        .grip
        .state(&session, Path::NULL)
        .ok()
        .filter(|s| s.is_active)
        .map(|s| s.current_state);
    hand.trigger = a
        .trigger
        .state(&session, Path::NULL)
        .ok()
        .filter(|s| s.is_active)
        .map(|s| s.current_state);
    hand.left_grip = a
        .left_grip
        .state(&session, Path::NULL)
        .ok()
        .filter(|s| s.is_active)
        .map(|s| s.current_state);
    if hand.trigger.is_some_and(|v| v > 0.6) {
        hand.firing = true;
    } else if hand.trigger.is_none_or(|v| v < 0.35) {
        hand.firing = false;
    }
    let pause = matches!(
        mode.state.mode,
        rc_game::menus::mode::Mode::Menu | rc_game::menus::mode::Mode::Freeze
    ) || mode.spatial_vendor
        || play.as_ref().is_some_and(|p| super::game::native_panel(p));
    if !hand.firing {
        hand.menu_consumed = false;
    }
    if pause {
        if !hand.menu_active {
            hand.menu_ready = false;
            hand.menu_click = false;
        }
        if !hand.firing {
            hand.menu_ready = true;
        }
        if hand.firing && hand.menu_ready {
            hand.menu_click = true;
            hand.menu_ready = false;
        }
        hand.menu_held = hand.firing;
        if hand.firing {
            hand.menu_consumed = true;
        }
    } else {
        hand.menu_click = false;
        hand.menu_held = false;
        if hand.menu_consumed && hand.firing {
            hand.trigger = None;
            jump.trigger_consumed = true;
        }
    }
    hand.menu_active = pause;
    let left_trigger = a
        .left_trigger
        .state(&session, Path::NULL)
        .ok()
        .filter(|s| s.is_active)
        .map(|s| s.current_state);
    if left_trigger.is_some_and(|v| v > 0.6) {
        hand.pack_modifier = true;
        hand.left_firing = true;
    } else if left_trigger.is_none_or(|v| v < 0.35) {
        hand.pack_modifier = false;
        hand.left_firing = false;
    }
    let movement = vector(&a.movement);
    let camera = vector(&a.camera);
    out.active = movement.is_some() || camera.is_some();
    let mut buttons = 0;
    for (bit, a) in &a.buttons {
        if held(a) {
            buttons |= *bit;
        }
    }
    let utility_press =
        buttons & (button::START | button::TRIANGLE) | if hand.firing { button::CIRCLE } else { 0 };
    let utility_edge = utility_press & !*utility_down != 0;
    *utility_down = utility_press;
    let puzzle = play
        .as_ref()
        .is_some_and(|p| p.game.hero.items.slot.id == 26 && p.game.hero.state == 0x72);
    if puzzle {
        // Sticks operate the native rings. Y, stick click and a fresh trigger press withdraw.
        // The trigger used to plug in must be released before it can cancel.
        let cancel = utility_edge && !*utility_consumed;
        hand.utility_cancel |= cancel;
        *utility_consumed |= cancel;
        hand.menu_click = false;
        hand.trigger = None;
        hand.pack_jump = false;
        *selection = HandSelect::default();
        rig.hold_snap(camera.unwrap_or_default().x);
        out.pad = encode(
            Vec2::ZERO,
            Vec2::ZERO,
            if hand.utility_cancel {
                button::TRIANGLE
            } else {
                menu_directions(movement.unwrap_or_default(), camera.unwrap_or_default())
            },
        );
        return;
    }
    if *utility_consumed {
        buttons &= !(button::START | button::TRIANGLE);
        hand.trigger = None;
        hand.menu_click = false;
        jump.trigger_consumed = hand.firing;
        if utility_press == 0 {
            *utility_consumed = false;
        }
    }
    let media = matches!(
        mode.state.mode,
        rc_game::menus::mode::Mode::Cutscene | rc_game::menus::mode::Mode::Movie
    );
    let start_down = buttons & button::START != 0;
    if media {
        if skip.media != Some(mode.state.mode) {
            skip.pending = false;
        }
        if start_down && !skip.down {
            skip.pending = true;
            skip.suppress = true;
        }
        out.skip_cutscene = skip.pending;
        buttons &= !button::START;
    } else {
        skip.pending = false;
        if skip.suppress {
            buttons &= !button::START;
        }
        if !start_down {
            skip.suppress = false;
        }
    }
    skip.down = start_down;
    skip.media = media.then_some(mode.state.mode);
    let gameplay = mode.free_movement();
    let shopping = mode.spatial_vendor && mode.state.mode == rc_game::menus::mode::Mode::Vendor;
    let mut movement = movement.unwrap_or_default();
    let mut camera = camera.unwrap_or_default();
    if hand.left_grip.is_some_and(|v| v > 0.6) {
        *left_squeeze = true;
    } else if hand.left_grip.is_none_or(|v| v < 0.35) {
        *left_squeeze = false;
    }
    if gameplay {
        // On the hoverboard (super::game turns the view with the board): no stick turn, the right stick holds the tricks,
        // the left trigger boosts (not R1), the right trigger fires Kalebo III's board weapon (○, as the other weapons:
        // super::blaster) and the left grip is L1.
        let board = play
            .as_ref()
            .is_some_and(|p| p.game.hero.mode == 0 && p.game.hero.group == rc_game::hero::hoverboard::GROUP);
        // The left hand, the same buttons whether Akimbo (super::akimbo) is on or not: the left stick's click opens its
        // ring (with Akimbo; X is the map), the left trigger fires its weapon (off the board: there the trigger boosts)
        // and pages its ring, and the left grip is R1 (crouch, pack jumps, flips) unless it holds a ledge or supports the
        // right hand's weapon; with Akimbo off the left trigger is R1 as well.
        let akimbo_on = options.as_ref().is_some_and(|o| o.akimbo) && !shopping;
        let left_ring = akimbo_on && buttons & button::SELECT != 0;
        buttons &= !button::SELECT;
        let left_weapon = akimbo_on && !board && akimbo.as_ref().is_some_and(|a| a.left != 0);
        if left_ring && hand.left_firing {
            left.consumed = true;
        } else if !hand.left_firing {
            left.consumed = false;
        }
        // The left trigger turns the left ring's page, as the right one does in the hand's.
        hand.left_page |= left_ring && hand.left_firing && !left.page_held;
        left.page_held = left_ring && hand.left_firing;
        out.left_fire = left_weapon && hand.left_firing && !left_ring && !left.consumed;
        if left_ring {
            // The left hand picks as the right does in its ring (from where it was at the click); the stick keeps walking.
            left.select.held = true;
            let at = tracked.iter().find_map(|(hand, pose, flags)| {
                (hand.0 == 0 && flags.pos_valid() && flags.pos_tracked()).then_some(pose.translation)
            });
            out.left_select = Some(left.select.direction(at, super::game::pose(views.as_deref()), rig.revision));
        } else if left.select.held {
            left.select = HandSelect::default();
        }
        let ledge = play.as_ref().is_some_and(|p| {
            p.game.hero.tracked.grabbed[0] || rc_game::hero::tracked::ledge_hold(p.game.hero.state)
        });
        // A squeeze that took the right weapon's foregrip is the support's until released.
        if !*left_squeeze {
            left.support = false;
        } else if !left.squeezed {
            let pose = |side: usize| tracked.iter().find_map(|(h, pose, _)| (h.0 == side).then_some(*pose));
            left.support = support.as_ref().is_some_and(|s| pose(1).zip(pose(0)).is_some_and(|(r, l)| s.reaches(r, l)));
        }
        left.squeezed = *left_squeeze;
        // With Akimbo off the left trigger is R1 too (it fires nothing).
        let pack = (*left_squeeze && !ledge && !left.support && !support.as_ref().is_some_and(|s| s.held))
            || (!akimbo_on && hand.left_firing);
        if pack {
            buttons |= button::R1;
        }
        // X, free in play, is the map (Select; Y is the pause), and B holds the native R2 (the hover strafe, the
        // Hydro-Pack's boost). Mounted they keep ○ and □ (a flown ship's missiles), disguised B keeps □.
        let face = buttons;
        if shopping {
            // Purchases are the pointer's click (super::vendor); the stick pages.
            let mut shop_buttons = buttons & button::TRIANGLE;
            if camera.x > 0.7 {
                shop_buttons |= button::RIGHT;
            }
            if camera.x < -0.7 {
                shop_buttons |= button::LEFT;
            }
            out.vendor = Some(encode(Vec2::ZERO, Vec2::ZERO, shop_buttons));
            hand.trigger = None;
            buttons &= !button::TRIANGLE;
            *selection = HandSelect::default();
        }
        if jump.shopping && !shopping && hand.firing {
            jump.trigger_consumed = true;
        }
        jump.shopping = shopping;
        let down = buttons & button::CROSS != 0;
        if !hand.firing {
            jump.trigger_consumed = false;
        }
        let disguised = play.as_ref().is_some_and(|p| p.game.hero.mode == 3 && p.game.hero.health > 0);
        // The trigger that put the disguise on may still be held: only a fresh press salutes.
        if disguised && !jump.disguised && hand.firing {
            jump.trigger_consumed = true;
        }
        jump.disguised = disguised;
        let tricks = if board { board_tricks(camera) } else { 0 };
        let boost = board && hand.pack_modifier;
        if down && !jump.pressed && pack {
            let pack_ready = play.as_ref().is_some_and(|p| {
                let h = &p.game.hero;
                h.mode == 0 && h.grounded() && matches!(h.state, 0 | 2 | 3 | 4 | 0x73)
            });
            if pack_ready {
                // Queued until the next fixed tick; native tracked transitions take priority over Cross.
                hand.pack_jump = true;
            }
        }
        jump.pressed = down;
        // Cross always follows A immediately, including held-A jump height and glide.
        // A trigger press already used (the shop, a menu, a utility cancel) fires nothing until released;
        // left-trigger pack/flip input leaves weapons independent.
        if jump.trigger_consumed {
            hand.trigger = None;
        }
        // Mounted (hero state 0x32): a turret (Gaspar's cannons, Hoven's and Batalia's turrets fire with ○ held) or a
        // flown ship (the vehicle record: Pokitaru's jet, Gemlik's ship and the fleet's fire their lasers with □ / L1
        // and their missiles with ○ / R1).
        let mounted = play.as_ref().filter(|p| p.game.hero.mode == 0 && p.game.hero.state == 0x32);
        let flying = mounted.is_some_and(|p| p.svc.vehicle.moby.is_some());
        let turret = mounted.is_some() && !flying;
        // Circle comes only from the trigger (the trigger-fired weapons, Giant Clank's missiles, turrets); X remains a
        // menu key, and the missile key while mounted.
        if mounted.is_none() {
            buttons &= !button::CIRCLE;
        }
        if hand.firing
            && !shopping
            && !jump.trigger_consumed
            && buttons & button::TRIANGLE == 0
            && play.as_ref().is_some_and(|p| {
                super::blaster::supported(p.game.hero.items.slot.id)
                    && !rc_game::hero::items::is_glove(p.game.hero.items.slot.id)
                    && p.game.hero.items.slot.state == 2
            })
            && tracked.iter().any(|(h, _, f)| {
                h.0 == 1 && f.pos_valid() && f.pos_tracked() && f.rot_valid() && f.rot_tracked()
            })
        {
            buttons |= button::CIRCLE;
        }
        // The trigger fires a turret (○ held) and a flown ship's lasers (□ held).
        if hand.firing && !shopping && !jump.trigger_consumed {
            if turret {
                buttons |= button::CIRCLE;
            } else if flying {
                buttons |= button::SQUARE;
            }
        }
        // Giant Clank's missiles (○ held) and the disguise's salute (○, held while the pose lasts) on the trigger.
        if hand.firing
            && !shopping
            && !jump.trigger_consumed
            && play.as_ref().is_some_and(|p| matches!(p.game.hero.mode, 2 | 3))
        {
            buttons |= button::CIRCLE;
        }
        // The disguise also salutes when the right hand does: raised open, to the brow or waving.
        let right = tracked.iter().find_map(|(h, pose, f)| {
            (h.0 == 1 && f.pos_valid() && f.pos_tracked() && f.rot_valid() && f.rot_tracked()).then_some(*pose)
        });
        if !disguised || buttons & button::TRIANGLE != 0 {
            jump.salute = Salute::default();
        } else if jump.salute.update(super::game::pose(views.as_deref()), right, hand.grip.unwrap_or(0.0)) {
            buttons |= button::CIRCLE;
        }
        // Mounted, △ leaves the turret or the ship (natively), it does not open the hand's quick select.
        let selecting = buttons & button::TRIANGLE != 0 && mounted.is_none();
        if selecting {
            rig.hold_snap(camera.x);
            // The same hand's trigger turns the page (a trigger already held when the ring opens does not); while the
            // ring is open it fires nothing, nor once it closes until released.
            if !selection.held {
                selection.page_held = hand.firing;
            }
            selection.held = true;
            if !hand.firing {
                selection.page_held = false;
            } else if !selection.page_held {
                hand.quick_page = 1;
                selection.page_held = true;
            }
            if hand.firing {
                jump.trigger_consumed = true;
            }
            hand.trigger = None;
            let right = tracked.iter().find_map(|(hand, pose, flags)| {
                (hand.0 == 1 && flags.pos_valid() && flags.pos_tracked())
                    .then_some(pose.translation)
            });
            // The right hand picks; the left stick keeps walking.
            out.quick_select =
                Some(selection.direction(right, super::game::pose(views.as_deref()), rig.revision));
        } else if selection.held {
            // Release confirms the last hand-selected sector.
            *selection = HandSelect::default();
        }
        if !selecting {
            if shopping || board {
                rig.hold_snap(camera.x);
            } else {
                rig.turn(camera.x, options.as_ref().is_some_and(|o| o.smooth_turn), time.delta_secs());
            }
        }
        camera = Vec2::ZERO;
        // Triangle keeps the native quick-select/context interaction priority.
        // □ (B) leaves the Hologuise disguise, and so does A (its states ignore ✕; ✕ stays held, so Ratchet does not
        // jump on his return). Clank's and Giant Clank's punches are physical (super::body_fists).
        let square = if disguised || mounted.is_some() { button::SQUARE } else { 0 };
        if disguised && buttons & button::CROSS != 0 {
            buttons |= button::SQUARE;
        }
        buttons &= button::CROSS
            | button::R1
            | button::CIRCLE
            | button::TRIANGLE
            | button::START
            | button::SELECT
            | square;
        if mounted.is_none() && !shopping {
            if face & button::CIRCLE != 0 {
                buttons |= button::SELECT;
            }
            if face & button::SQUARE != 0 && !disguised {
                buttons |= button::R2;
            }
        }
        if board {
            buttons &= !button::R1;
            buttons |= tricks;
            if boost {
                buttons |= button::SQUARE;
            }
            if *left_squeeze {
                buttons |= button::L1;
            }
        }
    } else {
        *selection = HandSelect::default();
        *jump = JumpInput::default();
        // A left trigger held into play (a menu's R1) fires the left hand's weapon only once released.
        *left = LeftHand { consumed: hand.left_firing, ..default() };
        hand.pack_jump = false;
        hand.quick_page = 0;
        rig.hold_snap(camera.x);
        if matches!(
            mode.state.mode,
            rc_game::menus::mode::Mode::Menu | rc_game::menus::mode::Mode::Freeze
        ) {
            // A confirms as the trigger does: a click on what the right hand points at (super::menu_pointer, which
            // sends the native ✕ on a page without pointer targets). Pointing is the selection: the sticks only
            // navigate a page the pointer cannot.
            let cross = buttons & button::CROSS != 0;
            if cross && !hand.menu_cross {
                hand.menu_click = true;
            }
            hand.menu_held |= cross;
            hand.menu_cross = cross;
            buttons &= !button::CROSS;
            if !hand.menu_targets {
                buttons |= menu_directions(movement, camera);
            }
            // The page shoulders (the map's previous / next page, the planet lists, shoulder-stepped lists): the left
            // grip is L1, the left trigger R1 as in play.
            if *left_squeeze {
                buttons |= button::L1;
            }
            if hand.pack_modifier {
                buttons |= button::R1;
            }
        }
    }
    if gameplay
        && play
            .as_ref()
            .is_some_and(|p| rc_game::hero::tracked::ledge_hold(p.game.hero.state))
    {
        movement = Vec2::ZERO;
    }
    if gameplay {
        if let Some(p) = play.as_ref() {
            if let Some((bolt, view)) = p
                .game
                .hero
                .tracked
                .crank_target
                .and_then(|id| p.game.mobys.mobys.get(id))
                .zip(p.game.hero.tracked.navigation)
            {
                let body = p.game.hero.position();
                let radial = Vec2::new(body[0] - bolt.position[0], body[1] - bolt.position[1]);
                let distance = radial.length();
                if distance > 0.1 {
                    let r = radial / distance;
                    let tangent = Vec2::new(-r.y, r.x);
                    let correction =
                        (p.game.hero.tracked.crank_radius - distance).clamp(-0.25, 0.25) * 1.3;
                    let desired = tangent * (movement.y + movement.x * 0.7) + r * correction;
                    let f = rc_game::hero::physics::to_f32x3(view.rows[0]);
                    let l = rc_game::hero::physics::to_f32x3(view.rows[1]);
                    movement = Vec2::new(
                        -desired.dot(Vec2::new(l[0], l[1])),
                        desired.dot(Vec2::new(f[0], f[1])),
                    );
                }
            }
        }
    }
    if hand.menu_targets
        && matches!(
            mode.state.mode,
            rc_game::menus::mode::Mode::Menu | rc_game::menus::mode::Mode::Freeze
        )
    {
        // Pointing selects on this page: the sticks stay out of it.
        (movement, camera) = (Vec2::ZERO, Vec2::ZERO);
    }
    out.pad = encode(movement, camera, buttons);
    if buttons != *previous {
        println!("VR_GAME controller_buttons={buttons:#06x} move={movement:?} camera={camera:?}");
        *previous = buttons;
    }
}

fn hand_select_menu(
    input: Res<GamePad>,
    mode: Res<crate::menu_render::MenuMode>,
    play: Option<ResMut<crate::gameplay::Play>>,
    mut hand: ResMut<HandInput>,
    saved: Option<ResMut<crate::gameplay::Persistent>>,
    menu: Option<ResMut<crate::menu_render::MenuRt>>,
    (mut page, mut was_open): (Local<usize>, Local<bool>),
) {
    let change = std::mem::take(&mut hand.quick_page);
    // The ring always opens on its first page, not on the one last used.
    let open = menu.as_ref().and_then(|m| m.qs.as_ref()).is_some_and(|q| q.open && !q.command_page);
    let reopened = open && !*was_open && *page != 0;
    *was_open = open;
    if change != 0 || reopened {
        if let (Some(mut saved), Some(mut menu)) = (saved, menu) {
            if let Some(qs) = menu.qs.as_mut().filter(|q| q.open && !q.command_page) {
                let owned: Vec<i32> = super::blaster::SELECTABLE
                    .into_iter()
                    .filter(|&id| saved.0.global.owned[id as usize] != 0)
                    .collect();
                let pages = owned.len().div_ceil(8).max(1);
                if reopened {
                    *page = 0;
                }
                *page = (*page as isize + change as isize).rem_euclid(pages as isize) as usize;
                for slot in 0..8 {
                    let id = owned.get(*page * 8 + slot).copied().unwrap_or(0);
                    saved.0.global.quick_select[slot] = id;
                    qs.entries[slot].item = id;
                    qs.entries[slot].icon =
                        qs.items.icon.get(id as usize).copied().unwrap_or(0) as i32;
                }
                qs.sel = -1;
                qs.arm = 1;
                println!(
                    "VR_GAME quick_select_page={}/{} items={:?}",
                    *page + 1,
                    pages,
                    saved.0.global.quick_select
                );
            }
        }
    }
    if !input.active || mode.state.mode != rc_game::menus::mode::Mode::Gameplay {
        return;
    }
    let (Some(direction), Some(mut play)) = (input.quick_select, play) else {
        return;
    };
    // Only the menu's copied axes receive the gesture, after physics has consumed a neutral stick.
    // Triangle remains untouched for the original contextual interactions and wheel confirmation.
    let axes = encode(direction, Vec2::ZERO, 0);
    play.game.pad.analog_copy[2] = rc_game::pad::axis(axes.lx);
    play.game.pad.analog_copy[3] = rc_game::pad::axis(axes.ly);
    play.game.pad.stick_active = direction != Vec2::ZERO;
    // The walking stick's d-pad bits stay out of the ring (VR sends no d-pad of its own).
    let dpad = button::UP | button::RIGHT | button::DOWN | button::LEFT;
    play.game.pad.held_unmirrored &= !dpad;
    play.game.pad.pressed_unmirrored &= !dpad;
}

/// The ring locks the pad (PAD+0x1cc = 2) because the PS2 picks with the left stick; in VR the right hand picks, so the
/// stick keeps walking.
fn keep_stick_with_hero(input: Res<GamePad>, play: Option<ResMut<crate::gameplay::Play>>) {
    if !input.active {
        return;
    }
    if let Some(mut play) = play.filter(|p| p.game.pad.lock == 2) {
        play.game.pad.lock = 0;
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
    fn requested_face_buttons_and_stick_clicks_reach_original_pad() {
        let profile = "/interaction_profiles/oculus/touch_controller";
        let expected = [
            ("/user/hand/right/input/a/click", button::CROSS),
            ("/user/hand/right/input/b/click", button::SQUARE),
            ("/user/hand/left/input/x/click", button::CIRCLE),
            ("/user/hand/left/input/y/click", button::START),
            ("/user/hand/left/input/thumbstick/click", button::SELECT),
            ("/user/hand/right/input/thumbstick/click", button::TRIANGLE),
        ];
        let bits = [
            button::CROSS,
            button::SQUARE,
            button::CIRCLE,
            button::TRIANGLE,
            button::SELECT,
            button::START,
            button::L3,
            button::R3,
        ];
        for (path, expected_bit) in expected {
            let pressed = bits
                .into_iter()
                .filter(|bit| button_path(*bit, profile) == Some(path))
                .fold(0, |mask, bit| mask | bit);
            let mut decoded = PadState::default();
            decoded.update(
                Some(&encode(Vec2::ZERO, Vec2::ZERO, pressed).bytes()),
                false,
            );
            assert_eq!(
                decoded.raw, expected_bit,
                "{path} must send exactly one PS2 button"
            );
            decoded.update(Some(&PadInput::neutral().bytes()), false);
            assert_eq!(decoded.raw, 0, "{path} must release cleanly");
        }
    }
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
                ..default()
            }),
        );
        assert_eq!(p.ly, 0);
        assert_eq!(
            p.buttons as u32 & (button::CROSS | button::CIRCLE),
            button::CROSS | button::CIRCLE
        );
    }
}
