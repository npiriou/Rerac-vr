//! Bevy's `FullscreenMaterial` caches each view's bind group until the view's texture changes, but
//! the bind group also holds the material's uniform buffer, which is reallocated when more views
//! carry the material. VR copies the window camera's scene fade, underwater and screen tints to both
//! eyes (`game::mirror_pass`): when a cinematic re-activated the window camera, an eye kept a bind
//! group on the old one-entry buffer and drew with the third view's offset, a validation error that
//! closed the game. When a material's buffer changes, its cached bind groups are dropped so that
//! Bevy creates them again on the new buffer the same frame.
use bevy::{
    core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialBindGroup},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems, extract_component::ComponentUniforms,
        render_resource::BufferId,
    },
};

pub struct FullscreenRefreshPlugin;
impl Plugin for FullscreenRefreshPlugin {
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).add_systems(
            Render,
            (
                refresh::<crate::scene_render::SceneFade>,
                refresh::<crate::fog_state::UnderwaterTint>,
                refresh::<crate::screen_tint::ScreenTint>,
            )
                .in_set(RenderSystems::PrepareResourcesFlush),
        );
    }
}

/// Exclusive, so the removal is applied before `PrepareBindGroups` creates the bind groups again.
fn refresh<T: FullscreenMaterial>(world: &mut World, mut last: Local<Option<BufferId>>) {
    let buffer = world
        .get_resource::<ComponentUniforms<T>>()
        .and_then(|u| u.uniforms().buffer())
        .map(|b| b.id());
    if buffer == *last {
        return;
    }
    *last = buffer;
    let views: Vec<Entity> = world
        .query_filtered::<Entity, With<FullscreenMaterialBindGroup<T>>>()
        .iter(world)
        .collect();
    for view in views {
        world.entity_mut(view).remove::<FullscreenMaterialBindGroup<T>>();
    }
}
