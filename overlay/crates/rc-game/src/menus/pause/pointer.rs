//! Pointing selects the original menu cursors; a click goes through their ordinary button actions.
use super::{Data, MenuAssets, MenuInput, MenuOut, MenuSound, PageMenu, func, lf, list_font, port};
use crate::{game_state::GameState, hud::text as wtext, pad::button};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Row(i32),
    Key(u32),
    Scroll(i32),
    Volume(i32),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub page: u32,
    pub widget: u32,
    pub rect: [f32; 4],
    pub action: Action,
    pub enabled: bool,
}
impl Target {
    pub fn contains(&self, point: [f32; 2]) -> bool {
        let [x, y, w, h] = self.rect;
        point[0] >= x && point[0] < x + w && point[1] >= y && point[1] < y + h
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub target: Option<Target>,
    pub point: [f32; 2],
    pub pressed: bool,
    pub held: bool,
}

impl PageMenu {
    /// Pixel rectangles follow the original widget layout, including its current animation and scroll.
    pub fn pointer_targets(&self, assets: &MenuAssets, gs: &GameState) -> Vec<Target> {
        let mut targets = Vec::new();
        if !self.active
            || self.current == 0
            || self.target != 0
            || matches!(self.kind, 0 | 1 | 0x14)
        {
            return targets;
        }
        let Some(page) = self.pages.get(&self.current) else {
            return targets;
        };
        for (slot, id) in page.widgets.iter().copied().enumerate() {
            let Some(w) = self.widgets.get(&id) else {
                continue;
            };
            if w.dflags & 4 != 0 || w.draw == 0 || (slot == 6 && !self.goodies) {
                continue;
            }
            let [x, y, width, height] = w.rect;
            if width <= 0 || height <= 0 {
                continue;
            }
            let mut push = |rect: [f32; 4], action, enabled| {
                // Clip to both the native panel and the menu's 512x416 canvas.
                let left = (x as f32 + rect[0]).max(x.max(0) as f32);
                let top = (y as f32 + rect[1]).max(y.max(0) as f32);
                let right = (x as f32 + rect[0] + rect[2]).min((x + width).min(512) as f32);
                let bottom = (y as f32 + rect[1] + rect[3]).min((y + height).min(416) as f32);
                if right > left && bottom > top {
                    targets.push(Target {
                        page: self.current,
                        widget: id,
                        rect: [left, top, right - left, bottom - top],
                        action,
                        enabled,
                    });
                }
            };
            let row_rect = |cy: f32, h: f32| [4.0, cy - h * 0.5, (width - 8) as f32, h];
            match &w.data {
                Data::List(list) => {
                    let (font, size) = list_font(list.flags);
                    let step = if list.flags & lf::STEP_FROM_FONT != 0 {
                        size + 3
                    } else {
                        height / (list.items.len() as i32 + 1)
                    };
                    if w.draw == func::PLANET_LIST_DRAW {
                        let mut win = wtext::Window::new(
                            4,
                            (height - 4) as i16,
                            0,
                            (width - 2) as i16,
                            4,
                            (step - size / 2 - list.scroll) as i16,
                            (size + 2) as i16,
                            0,
                        );
                        for (i, item) in list.items.iter().enumerate() {
                            let mut measured = win;
                            wtext::layout(
                                &mut measured,
                                assets.msg_in(item.label as i32, self.text_swap == 2),
                                -1,
                                &assets.hud.glyphs[font as usize],
                                list.cursor != i as i32,
                            );
                            let top = win.y_start as f32;
                            let mut next = top + measured.height as f32;
                            if item.sublabel != 0 {
                                next += step as f32;
                            }
                            push(
                                [
                                    2.0,
                                    top - 2.0,
                                    (width - 4) as f32,
                                    (next - top + 8.0).max(size as f32),
                                ],
                                Action::Row(i as i32),
                                item.action != 0,
                            );
                            win.y_start = (next + 8.0) as i16;
                        }
                    } else {
                        let mut top = step - size / 2 - 1;
                        for (i, item) in list.items.iter().enumerate() {
                            let span = if item.sublabel != 0 {
                                step + size
                            } else {
                                size
                            };
                            push(
                                [4.0, (top - 4) as f32, (width - 8) as f32, (span + 8) as f32],
                                Action::Row(i as i32),
                                item.action != 0,
                            );
                            top += step * if item.sublabel != 0 { 2 } else { 1 };
                        }
                    }
                }
                Data::Grid(grid) => {
                    let Some(frame) = self
                        .frames
                        .as_ref()
                        .and_then(|f| f.slots.as_ref())
                        .and_then(|s| s.get(w.moby.max(0) as usize))
                    else {
                        continue;
                    };
                    let p = |k: usize| [0, 1, 2].map(|i| f32::from_bits(frame.corners[k][i]));
                    let distance = |a: [f32; 3], b: [f32; 3]| {
                        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2))
                            .sqrt()
                    };
                    let (sw, sh) = (distance(p(1), p(0)), distance(p(2), p(0)));
                    if sw.max(sh) < 0.001 || grid.cols <= 0 || grid.rows <= 0 {
                        continue;
                    }
                    let [cw, ch] = grid.cell;
                    let x0 = if grid.cols < 2 {
                        (sw - cw) * 0.5
                    } else {
                        grid.margin[0]
                    };
                    let dx = if grid.cols < 2 {
                        0.0
                    } else {
                        cw + ((sw - 2.0 * grid.margin[0]) - cw * grid.cols as f32)
                            / (grid.cols - 1) as f32
                    };
                    let (y0, dy) = if grid.flags & 2 != 0 {
                        (grid.margin[1], ch + 0.15)
                    } else if grid.rows < 2 {
                        ((sh - ch) * 0.5, 0.0)
                    } else {
                        (
                            grid.margin[1],
                            ch + ((sh - 2.0 * grid.margin[1]) - ch * grid.rows as f32)
                                / (grid.rows - 1) as f32,
                        )
                    };
                    let scale = width.max(height) as f32 / sw.max(sh);
                    for (i, cell) in grid.cells.iter().enumerate() {
                        let shown = if cell.kind == 0 {
                            gs.global.owned.get(cell.item.max(0) as usize)
                        } else {
                            gs.global.flags.get(cell.item.max(0) as usize)
                        }
                        .is_some_and(|v| *v != 0);
                        if shown {
                            push(
                                [
                                    (x0 + (i as i32 % grid.cols) as f32 * dx) * scale - 3.0,
                                    (y0 + (i as i32 / grid.cols) as f32 * dy) * scale - 3.0,
                                    cw * scale + 6.0,
                                    ch * scale + 6.0,
                                ],
                                Action::Row(i as i32),
                                grid.flags & 1 == 0,
                            );
                        }
                    }
                }
                Data::Toggle(t) => {
                    for i in 0..t.entries.len() {
                        let cy = (height / (t.entries.len() as i32 + 1) * (i as i32 + 1)) as f32;
                        push(row_rect(cy, 24.0), Action::Row(i as i32), true);
                    }
                }
                Data::Camera(c) => {
                    for i in 0..c.entries.len() {
                        let cy = (height / (c.entries.len() as i32 + 1) * (i as i32 + 1)) as f32;
                        push(row_rect(cy, 24.0), Action::Row(i as i32), true);
                    }
                }
                Data::Port(p) => {
                    let shown = port::VISIBLE.min(port::ENTRIES.len());
                    let step = height / (shown as i32 + 1);
                    let top = (p.top.max(0) as usize).min(port::ENTRIES.len() - shown);
                    for i in 0..shown {
                        push(
                            row_rect((step * (i as i32 + 1)) as f32, 24.0),
                            Action::Row((top + i) as i32),
                            true,
                        );
                    }
                    let gap = (step - 8).max(12) as f32;
                    if top > 0 {
                        push(
                            [4.0, 0.0, (width - 8) as f32, gap],
                            Action::Scroll(-1),
                            true,
                        );
                    }
                    if top + shown < port::ENTRIES.len() {
                        push(
                            [4.0, height as f32 - gap, (width - 8) as f32, gap],
                            Action::Scroll(1),
                            true,
                        );
                    }
                }
                Data::Sound { .. } => {
                    for row in 0..3 {
                        let cy = (height / 4 * (row + 1)) as f32;
                        push(row_rect(cy, 24.0), Action::Row(row), true);
                        if row < 2 {
                            push(
                                [
                                    (width / 2 + 9) as f32,
                                    cy - 10.0,
                                    (width - (width / 2 + 0x4a)).max(1) as f32,
                                    20.0,
                                ],
                                Action::Volume(row),
                                true,
                            );
                        }
                    }
                }
                Data::Quit => {
                    push(
                        row_rect((height - 0x28 + 8) as f32, 20.0),
                        Action::Key(button::TRIANGLE),
                        true,
                    );
                    push(
                        row_rect((height - 0x14 + 8) as f32, 20.0),
                        Action::Key(button::CIRCLE),
                        true,
                    );
                }
                Data::IconList(list) => {
                    for i in 0..list.cells.len() {
                        push(
                            [
                                (width as f32 - 32.0) * 0.5,
                                (list.y + i as i32 * 0x260) as f32 / 16.0,
                                32.0,
                                32.0,
                            ],
                            Action::Row(i as i32),
                            true,
                        );
                    }
                }
                Data::Confirm => push(
                    [4.0, 4.0, (width - 8) as f32, (height - 8) as f32],
                    Action::Key(button::CROSS),
                    true,
                ),
                _ => {}
            }
        }
        targets
    }
    pub(super) fn apply_pointer(
        &mut self,
        pointer: Input,
        inp: &mut MenuInput,
        gs: &mut GameState,
        out: &mut MenuOut,
    ) {
        let Some(target) = pointer
            .target
            .filter(|t| t.page == self.current && self.target == 0 && t.contains(pointer.point))
        else {
            return;
        };
        let Some(page) = self.pages.get(&self.current) else {
            return;
        };
        if !page.widgets.contains(&target.widget) {
            return;
        }
        let old_focus = page.focus;
        if old_focus != target.widget {
            self.call_leave(old_focus, out, gs);
            if let Some(page) = self.pages.get_mut(&self.current) {
                page.focus = target.widget;
                page.pending = 0;
            }
            self.call_enter(target.widget, true, gs);
        }
        let Some(widget) = self.widgets.get_mut(&target.widget) else {
            return;
        };
        let row = match target.action {
            Action::Row(i) | Action::Volume(i) => Some(i),
            _ => None,
        };
        let cursor = match &mut widget.data {
            Data::List(l) => Some(&mut l.cursor),
            Data::Grid(g) => Some(&mut g.cursor),
            Data::Toggle(t) => Some(&mut t.cursor),
            Data::Camera(c) => Some(&mut c.cursor),
            Data::Sound { cursor } => Some(cursor),
            Data::Port(p) => Some(&mut p.cursor),
            Data::IconList(l) => Some(&mut l.cursor),
            _ => None,
        };
        let mut changed = old_focus != target.widget;
        if let Some((cursor, row)) = cursor.zip(row) {
            changed |= *cursor != row;
            *cursor = row;
        }
        if changed {
            out.sounds.push(MenuSound::Cursor);
        }
        // A resting stick must not shift the option under a pointing controller.
        let directions = button::UP | button::DOWN | button::LEFT | button::RIGHT;
        inp.pressed &= !directions;
        inp.pressed_u &= !directions;
        inp.held &= !directions;
        inp.held_u &= !directions;
        if !target.enabled {
            return;
        }
        match target.action {
            Action::Scroll(direction) if pointer.pressed => {
                if let Data::Port(p) = &mut widget.data {
                    p.top = (p.top + direction)
                        .clamp(0, port::ENTRIES.len().saturating_sub(port::VISIBLE) as i32);
                    p.cursor = if direction < 0 {
                        p.top
                    } else {
                        (p.top + port::VISIBLE as i32 - 1).min(port::ENTRIES.len() as i32 - 1)
                    };
                    out.sounds.push(MenuSound::Cursor);
                }
            }
            Action::Volume(row) if pointer.held => {
                let value = (((pointer.point[0] - target.rect[0]) / target.rect[2]).clamp(0.0, 1.0)
                    * 1024.0)
                    .round() as i32;
                let volume = if row == 0 {
                    &mut gs.global.effects_volume
                } else {
                    &mut gs.global.music_volume
                };
                if *volume != value {
                    *volume = value;
                    out.sound_settings = true;
                }
            }
            Action::Row(_) | Action::Key(_) if pointer.pressed => {
                let key = if let Action::Key(key) = target.action {
                    key
                } else {
                    button::CROSS
                };
                inp.pressed |= key;
                inp.raw_pressed |= key;
                inp.pressed_u |= key;
                inp.held |= key;
                inp.held_u |= key;
            }
            _ => {}
        }
    }
}
