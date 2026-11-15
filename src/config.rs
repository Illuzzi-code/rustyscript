// config.rs — Configuración estática del mod.
// En esta fase las opciones son constantes en tiempo de compilación.
// La Fase 4 añadirá lectura desde archivo .toml.

use crate::controls::vk;

/// Tecla para restaurar salud
pub const KEY_RESTORE_HEALTH: u8 = vk::F6;

/// Tecla para restaurar armadura  
pub const KEY_RESTORE_ARMOR: u8 = vk::F7;

/// Tecla para recargar scripts (Fase 4)
pub const KEY_RELOAD_SCRIPTS: u8 = vk::F9;

/// Tiempo máximo de CPU por frame para el intérprete de scripts (microsegundos)
pub const SCRIPT_FRAME_BUDGET_US: u64 = 2000;
