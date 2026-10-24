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
    pub pack_jump: bool,
    pub menu_click: bool,
    pub menu_held: bool,
    pub utility_cancel: bool,
    menu_ready: bool,
    menu_active: bool,
    menu_consumed: bool,
    quick_page: i8,
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
    page_held: bool,
}
impl HandSelect {
    fn direction(&mut self, hand: Option<Vec3>, head: Option<Transform>, revision: u64) -> Vec2 {
        if self.revision != revision {
            *self = Self {
                held: true,
                revision,
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
            left_grip: set.create_action("left_grip", "Support barrel / grab ledge", &[])?,
            left_trigger: set.create_action(
                "pack_modifier",
                "R1 / pack jumps / side flips",
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
    (mut utility_down, mut utility_consumed): (Local<u32>, Local<bool>),
) {
    *out = GamePad::default();
    hand.grip = None;
    hand.left_grip = None;
    hand.trigger = None;
    if !focus.0 {
        hand.firing = false;
        hand.menu_click = false;
        hand.menu_held = false;
        hand.menu_ready = false;
        hand.menu_active = false;
        hand.menu_consumed = true;
        hand.pack_modifier = false;
        hand.pack_jump = false;
        hand.quick_page = 0;
        *jump = JumpInput::default();
        *skip = SkipInput::default();
        *previous = 0;
        *selection = HandSelect::default();
        hand.utility_cancel = false;
        *utility_down = 0;
        *utility_consumed = false;
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
    } else if left_trigger.is_none_or(|v| v < 0.35) {
        hand.pack_modifier = false;
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
    if gameplay {
        if hand.pack_modifier {
            buttons |= button::R1;
        }
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
        if down && !jump.pressed && hand.pack_modifier {
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
        // Circle comes only from the trigger (the trigger-fired weapons, Giant Clank's missiles); X remains a menu key.
        buttons &= !button::CIRCLE;
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
        // Giant Clank's missiles (○ held) on the trigger.
        if hand.firing
            && !shopping
            && !jump.trigger_consumed
            && play.as_ref().is_some_and(|p| p.game.hero.mode == 2)
        {
            buttons |= button::CIRCLE;
        }
        let selecting = buttons & button::TRIANGLE != 0;
        if selecting {
            rig.hold_snap(camera.x);
            selection.held = true;
            if camera.x.abs() < 0.25 {
                selection.page_held = false;
            }
            if camera.x.abs() > 0.7 && !selection.page_held {
                hand.quick_page = if camera.x > 0.0 { 1 } else { -1 };
                selection.page_held = true;
            }
            let right = tracked.iter().find_map(|(hand, pose, flags)| {
                (hand.0 == 1 && flags.pos_valid() && flags.pos_tracked())
                    .then_some(pose.translation)
            });
            out.quick_select =
                Some(selection.direction(right, super::game::pose(views.as_deref()), rig.revision));
            movement = Vec2::ZERO;
        } else if selection.held {
            // Release confirms the last hand-selected sector without leaking the locomotion stick.
            movement = Vec2::ZERO;
            *selection = HandSelect::default();
            if shopping { rig.hold_snap(camera.x) } else { rig.snap(camera.x) }
        } else {
            if shopping { rig.hold_snap(camera.x) } else { rig.snap(camera.x) }
        }
        camera = Vec2::ZERO;
        // Triangle keeps the native quick-select/context interaction priority.
        // □ is the Hologuise's exit. Clank's and Giant Clank's punches are physical (super::body_fists).
        let square = if play.as_ref().is_some_and(|p| p.game.hero.mode == 3) { button::SQUARE } else { 0 };
        buttons &= button::CROSS
            | button::R1
            | button::CIRCLE
            | button::TRIANGLE
            | button::START
            | button::SELECT
            | square;
    } else {
        *selection = HandSelect::default();
        *jump = JumpInput::default();
        hand.pack_jump = false;
        hand.quick_page = 0;
        rig.hold_snap(camera.x);
        if matches!(
            mode.state.mode,
            rc_game::menus::mode::Mode::Menu | rc_game::menus::mode::Mode::Freeze
        ) {
            buttons |= menu_directions(movement, camera);
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
    mut page: Local<usize>,
) {
    let change = std::mem::take(&mut hand.quick_page);
    if change != 0 {
        if let (Some(mut saved), Some(mut menu)) = (saved, menu) {
            if let Some(qs) = menu.qs.as_mut().filter(|q| q.open && !q.command_page) {
                let owned: Vec<i32> = super::blaster::SELECTABLE
                    .into_iter()
                    .filter(|&id| saved.0.global.owned[id as usize] != 0)
                    .collect();
                let pages = owned.len().div_ceil(8).max(1);
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
