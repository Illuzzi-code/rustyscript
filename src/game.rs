// game.rs — Todas las direcciones de memoria verificadas del binario.
//
// REGLA: ninguna dirección entra aquí sin ficha de evidencia en
// docs/evidence/<nombre>.md con firma de bytes, xrefs y nivel de confianza.
//
// Versión objetivo: GTA San Andreas 1.0 US
// SHA-256: F01A00CE950FA40CA1ED59DF0E789848C6EDCF6405456274965885D0929343AC
// Tamaño : 14 383 616 bytes   ImageBase: 0x00400000   Sin ASLR.

#![allow(dead_code)]

// ── Funciones del juego ─────────────────────────────────────────────────────

/// CGame::Process — bucle principal del juego.
/// Firma: 83 EC 0C 53 56 57 E8 E5 5E 00 00 B9 78 29 B7 00
/// Un único caller en HOOK_CALL_SITE.
/// Convención: __cdecl, sin argumentos.
/// Confianza: ALTA  Prueba: hook activo y funcionando
pub const CGAME_PROCESS: usize = 0x53BEE0;

/// CMessages::AddMessageJumpQ — mensaje en esquina inferior izquierda.
/// Firma: 81 EC 20 03 00 00 56 8B B4 24 28 03 00 00 68 90
/// Convención: __cdecl  args: (*const u8, u32 time_ms, u32 flags, u32 add_to_briefs)
/// AVISO: no copia el texto; guarda el puntero en DAT_00C1A7F0 (0xC1A7F0) y el
/// render lo usa en frames posteriores.  El buffer debe ser persistente.
/// Confianza: ALTA  Prueba: docs/evidence/add_message_jumpq.md
pub const ADD_MESSAGE_JUMPQ: usize = 0x69F1E0;

/// CHud::SetHelpMessage — caja amarilla de ayuda en pantalla.
/// Firma: A0 C0 AE BA 00 84 C0 0F 85 4F 01 00 00
/// Convención: __cdecl  args: (*const u8, quick: u8, permanent: u8, beep: u8)
/// Confianza: ALTA  Prueba: pendiente (texto visible)
pub const SET_HELP_MESSAGE: usize = 0x588BE0;

/// CCheat vehicle spawn — función core llamada por todos los stubs.
/// Todos los stubs 0x43A4A0–0x43A680 llaman a esta.
/// Convención: __cdecl  args: (model_id: i32)
/// Confianza: MEDIA — pendiente prueba en juego.
/// NO USAR hasta que el usuario confirme.
pub const VEHICLE_CHEAT_CORE: usize = 0x43A0B0;

// ── Stubs de spawn de cheats (funciones hoja de 12 bytes) ─────────────────
// Patrón: 68 <model> E8 <rel> 59 C3
// Confianza: ALTA para todos (en tabla de hashes de cheats en 0x8A5B9C)
// Prueba en juego: pendiente
pub const CHEAT_SPAWN_RHINO:    usize = 0x43A4A0;  // model 432
pub const CHEAT_SPAWN_HOTRING:  usize = 0x43A4D0;  // model 502
pub const CHEAT_SPAWN_HYDRA:    usize = 0x43A530;  // model 520
pub const CHEAT_SPAWN_HUNTER:   usize = 0x43A550;  // model 425
pub const CHEAT_SPAWN_MONSTER:  usize = 0x43A4F0;  // model 442
pub const CHEAT_SPAWN_STRETCH:  usize = 0x43A500;  // model 409
pub const CHEAT_SPAWN_CADDY:    usize = 0x43A520;  // model 457

// ── Variables globales del juego ───────────────────────────────────────────

/// CWorld::Players[0] — puntero a CPlayerInfo struct.
/// *(0xB74490) = dirección base de CPlayerInfo.
/// *(CPlayerInfo + 0x00) = puntero al CPed del jugador.
/// Confianza: ALTA  Xrefs: 376  Prueba: in-game confirmada
pub const CWORLD_PLAYERS: usize = 0xB74490;

/// CClock::ms_nGameClockHours — hora del día (u8, 0–23).
/// Escritura directa. Confianza: ALTA  Xrefs: 5
pub const CLOCK_HOURS: usize = 0xB70153;

/// CClock::ms_nGameClockMinutes — minuto del día (u8, 0–59).
/// Escritura directa. Confianza: ALTA  Xrefs: 5
pub const CLOCK_MINUTES: usize = 0xB70152;

/// CWeather::ms_OldWeatherType (u16). Confianza: ALTA  Xrefs: 5
pub const WEATHER_OLD: usize = 0xC81320;

/// CWeather::ms_NewWeatherType (u16). Confianza: ALTA  Xrefs: 5
/// Escribir el mismo valor a OLD y NEW fuerza el cambio inmediato.
pub const WEATHER_NEW: usize = 0xC81324;

// ── Offsets dentro de CPed ─────────────────────────────────────────────────

/// CPed::m_fHealth (f32) — confirmado in-game: 23.6→100.1
pub const PED_OFFSET_HEALTH:     usize = 0x540;

/// CPed::m_fMaxHealth (f32) — confirmado in-game: 100.1
pub const PED_OFFSET_MAX_HEALTH: usize = 0x544;

/// CPed::m_fArmor (f32) — confirmado in-game: 0.0→100.0
pub const PED_OFFSET_ARMOR:      usize = 0x548;

/// CPlaceable posición X (f32) — confirmado por LEA EAX,[ECX+0x44]
pub const PED_OFFSET_X: usize = 0x44;
pub const PED_OFFSET_Y: usize = 0x48;
pub const PED_OFFSET_Z: usize = 0x4C;

/// CPlayerPed::m_pWanted — puntero a CWanted (u32 ptr).
/// [ped+0x484]+0x08 = m_nWantedLevel (u32, 0–6).
/// Confianza: MEDIA — pendiente confirmación en juego.
pub const PED_OFFSET_WANTED_PTR: usize = 0x484;

// ── Offsets dentro de CPlayerInfo ─────────────────────────────────────────

/// CPlayerInfo::m_nMoney (i32) — offset desde base de CPlayerInfo.
/// En tiempo real: *(*(0xB74490) + 0xB8)  — Confianza: MEDIA
pub const PLAYERINFO_OFFSET_MONEY: usize = 0xB8;

// ── Punto de hook ──────────────────────────────────────────────────────────

/// El E8 (CALL rel32) en 0x53E981 es el único call a CGame::Process.
/// El hook parcheó este E8. File offset: 0x13DD81.
pub const HOOK_CALL_SITE: usize = 0x53E981;

// ── IDs de clima ───────────────────────────────────────────────────────────
pub mod weather {
    pub const EXTRA_SUNNY: u16 = 0;
    pub const SUNNY:       u16 = 1;
    pub const CLOUDY:      u16 = 2;
    pub const RAINY:       u16 = 3;
    pub const STORMY:      u16 = 4;
    pub const FOGGY:       u16 = 9;
    pub const OVERCAST:    u16 = 10;
    pub const DESERT:      u16 = 11;
}
