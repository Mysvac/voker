use core::sync::atomic::Ordering;

use voker_app::prelude::*;
use voker_ecs::borrow::Res;
use voker_ecs::world::World;
use voker_ecs::{borrow::ResMut, derive::Resource};
use voker_os::sync::atomic::AtomicU32;

use crate::{
    DEFAULT_MAX_HISTORY_LENGTH, Diagnostic, DiagnosticPath, DiagnosticsStore, RegisterDiagnostic,
};

// -----------------------------------------------------------------------------
// EntityCount & EntityCountPlugin

#[derive(Debug, Default, Resource)]
pub struct EntityCount(AtomicU32);

impl EntityCount {
    pub fn get(&self) -> u32 {
        self.0.load(Ordering::Relaxed)
    }
}

fn count_entities(world: &World, count: Res<EntityCount>) {
    let num = world.entity_count();
    count.0.store(num as u32, Ordering::Relaxed);
}

#[derive(Debug, Default)]
pub struct EntityCountPlugin;

impl Plugin for EntityCountPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EntityCount>();
        app.add_system(PreUpdate, count_entities);
    }
}

// -----------------------------------------------------------------------------
// EntityCountDiagnosticsPlugin

/// Adds `entity_count` diagnostics to an app.
#[derive(Debug)]
pub struct EntityCountDiagnosticsPlugin {
    /// Number of samples kept in history.
    pub max_history_length: usize,
}

impl EntityCountDiagnosticsPlugin {
    /// Number of currently alive entities.
    pub const ENTITY_COUNT: DiagnosticPath = DiagnosticPath::new("entity_count");
}

impl Default for EntityCountDiagnosticsPlugin {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_HISTORY_LENGTH)
    }
}

impl EntityCountDiagnosticsPlugin {
    /// Creates a plugin using the provided history length.
    pub fn new(max_history_length: usize) -> Self {
        Self { max_history_length }
    }

    fn diagnostic_system(count: Res<EntityCount>, mut store: ResMut<DiagnosticsStore>) {
        let value = count.get() as f64;
        store.add_measurement(&Self::ENTITY_COUNT, value);
    }
}

impl Plugin for EntityCountDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.register_diagnostic(
            Diagnostic::new(Self::ENTITY_COUNT).with_max_history_length(self.max_history_length),
        )
        .add_systems(Update, Self::diagnostic_system);
    }
}
