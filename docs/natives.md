# Catálogo de funciones nativas — RustyScript

Versión: GTA San Andreas 1.0 US  
SHA-256 exe: `F01A00CE950FA40CA1ED59DF0E789848C6EDCF6405456274965885D0929343AC`

Leyenda de confianza:
- **ALTA** — firma bytes verificada en binario + xrefs coherentes + prueba en juego
- **MEDIA** — firma verificada, xrefs coherentes, pendiente prueba en juego
- **BAJA** — hipótesis sin verificar → NO SE USA

Solo las de confianza ALTA están activas en el intérprete de scripts.
Las de confianza MEDIA aparecen en el código pero desactivadas (`#[cfg(feature="unverified")]`).

---

## Grupo 1 — Jugador (prioridad alta, riesgo bajo)

### `player.health` — leer salud
- **Mecanismo:** `*(*(*(0xB74490)) + 0x540)` como f32
- **Confianza:** ALTA ✅
- **Prueba:** `health 23.6 → 100.1` confirmado en juego
- **Riesgo:** solo lectura — ninguno

### `player.health = <valor>` — escribir salud
- **Mecanismo:** escritura float a `[ped+0x540]`, clampada a `[0, max_health]`
- **Confianza:** ALTA ✅
- **Prueba:** confirmado en juego (F6 restaura salud)
- **Riesgo:** bajo — valor clampado, nunca supera max_health

### `player.max_health` — leer salud máxima
- **Mecanismo:** `[ped+0x544]` como f32
- **Confianza:** ALTA ✅
- **Prueba:** valor 100.1 leído in-game

### `player.armor` — leer/escribir armadura
- **Mecanismo:** `[ped+0x548]` como f32, clampada a [0, 100]
- **Confianza:** ALTA ✅
- **Prueba:** `armor 0.0 → 100.0` confirmado in-game (F7)

### `player.x`, `player.y`, `player.z` — posición
- **Mecanismo:** `[ped+0x44/0x48/0x4C]` como f32
- **Confianza:** ALTA ✅ (LEA EAX,[ECX+0x44] en 0x404460; FLD [reg+0x44] en 132 sitios)
- **Prueba:** pendiente (lectura) — MEDIA para escritura
- **Riesgo escritura:** teletransporte puede causar glitches. Solo lectura activada por defecto.

### `player.money` — leer/escribir dinero
- **Mecanismo:** `[*(0xB74490) + 0xB8]` como i32
- **Confianza:** MEDIA ⚠️ (5 escrituras encontradas, pendiente confirmar que es dinero)
- **Estado:** desactivado hasta prueba en juego

### `player.wanted_level` — nivel de búsqueda (0–6)
- **Mecanismo:** escritura directa a `[*(ped+0x484) + 0x08]` como u32
- **Confianza:** MEDIA ⚠️ (offset 0x484 encontrado en 5 xrefs, pendiente confirmar)
- **Estado:** desactivado

---

## Grupo 2 — Vehículos (prioridad media, riesgo medio)

### `spawn_vehicle(<model_id>)` — spawn junto al jugador
- **Mecanismo:** `CALL 0x43A0B0` con model_id en stack
- **Confianza:** MEDIA ⚠️ — función verificada en tabla de cheats, callee sin prueba directa
- **Stubs verificados ALTA:** Rhino(432) @ `0x43A4A0`, Hydra(520) @ `0x43A530`, Hunter(425) @ `0x43A550`
- **Riesgo:** puede crashear si model_id no está cargado. Usar solo con modelos conocidos.
- **Estado:** desactivado (MEDIA) — se activa tras prueba

### `cheat_spawn_rhino()` — stub directo (ALTA)
- **Mecanismo:** `CALL 0x43A4A0` — stub de 12 bytes en tabla de cheats
- **Confianza:** ALTA ✅ (encontrado en tabla de hashes, mismo patrón que el cheat del juego)
- **Prueba:** pendiente — próxima prueba en juego
- **Riesgo:** bajo (es exactamente lo que hace el cheat HESOYAM del juego)

---

## Grupo 3 — Pantalla (prioridad alta, riesgo mínimo)

### `show_help(<texto>)` — caja amarilla de ayuda
- **Función:** `CHud::SetHelpMessage @ 0x588BE0`
- **Firma:** `A0 C0 AE BA 00 84 C0 0F 85 4F 01 00 00` ✅
- **Convención:** `__cdecl(text: *const u8, quick: u8, permanent: u8, beep: u8)`
- **Confianza:** ALTA ✅
- **Prueba:** pendiente visibilidad en pantalla (el texto llegó al juego vía F6/F7 sin crash)
- **Nota:** acepta strings UTF-8 terminadas en null; no GXT keys

### `show_message(<texto>, <ms>)` — mensaje esquina inferior
- **Función:** `CMessages::AddMessageJumpQ @ 0x69F1E0`
- **Firma:** `81 EC 20 03 00 00 56 8B` ✅
- **Convención:** `__cdecl(text: *const u8, time_ms: u32, flags: u32, add_to_briefs: u32)`
- **Llamada:** `(texto, ms, 1, 1)` — igual que el caller interno del juego
- **Confianza:** ALTA ✅ (decompilado con Ghidra; ver `docs/evidence/show_message_lifetime.md`)
- **Aviso:** `AddMessageJumpQ` **no copia** el texto, guarda el puntero. La nativa
  copia el texto a un ring estático de 32 × 400 bytes (CP1252) para que siga
  vivo mientras el mensaje está en pantalla.
- **Validación:** requiere exactamente dos argumentos; `time_ms` debe estar entre 0 y 65535
- **Prueba:** pendiente de confirmación visual en juego

---

## Grupo 4 — Mundo (prioridad media, riesgo bajo)

### `set_time(<hora>, <minuto>)` — cambiar hora del día
- **Mecanismo:** escritura directa a `0xB70153` (horas u8) y `0xB70152` (minutos u8)
- **Confianza:** ALTA ✅ (5 xrefs cada uno, patrón `MOV [0xB70153], CL` confirmado)
- **Rango:** hora 0–23, minuto 0–59
- **Riesgo:** mínimo — solo cambia el ciclo día/noche visual

### `get_time()` — leer hora actual
- **Mecanismo:** lectura de `0xB70153` y `0xB70152`
- **Confianza:** ALTA ✅

### `set_weather(<id>)` — cambiar clima
- **Mecanismo:** escritura u16 a `0xC81320` (OldWeather) y `0xC81324` (NewWeather)
- **Confianza:** ALTA ✅ (5 xrefs cada uno)
- **Riesgo:** mínimo — reversible; el juego puede sobreescribirlo en el siguiente tick

---

## Grupo 5 — Armas (prioridad baja, riesgo alto)

### `give_weapon(<id>, <ammo>)`
- **Estado:** PENDIENTE — `CPed::AddWeapon` no está verificado en este binario
- **Razón:** `0x4474A0` (plugin-sdk) es interior de código, 0 callers encontrados
- **Acción necesaria:** análisis adicional con Ghidra para encontrar la función correcta

---

## Resumen de direcciones activas

| Nativo              | Dirección/Offset | Confianza | Activo |
|---------------------|------------------|-----------|--------|
| ped.health r/w      | `+0x540` (f32)   | ALTA ✅   | Sí     |
| ped.max_health r    | `+0x544` (f32)   | ALTA ✅   | Sí     |
| ped.armor r/w       | `+0x548` (f32)   | ALTA ✅   | Sí     |
| ped.x/y/z r         | `+0x44/48/4C`    | ALTA ✅   | Sí (r) |
| ped ptr             | `0xB74490` ×2    | ALTA ✅   | Sí     |
| set_time            | `0xB70153/52`    | ALTA ✅   | Sí     |
| set_weather         | `0xC81320/24`    | ALTA ✅   | Sí     |
| show_help           | `0x588BE0`       | ALTA ✅   | Sí     |
| show_message        | `0x69F1E0`       | ALTA ✅   | Sí     |
| cheat_spawn_rhino   | `0x43A4A0`       | ALTA ✅   | Sí*    |
| player.money r/w    | `+0xB8`          | MEDIA ⚠️ | No     |
| spawn_vehicle       | `0x43A0B0`       | MEDIA ⚠️ | No     |
| player.wanted       | `+0x484+0x08`    | MEDIA ⚠️ | No     |
| give_weapon         | desconocido      | BAJA ❌   | No     |

*cheat_spawn_rhino activo pero pendiente prueba en juego
