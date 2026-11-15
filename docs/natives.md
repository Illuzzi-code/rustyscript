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
- **Cadena verificada:** `CWorld::Players` (`0xB7CD98`) → `+0x00` `m_pPed` →
  `+0x480` `m_pPlayerData` → `+0x00` `m_pWanted` → `+0x2C` `m_nWantedLevel`
- **Escritura correcta:** `CPed::SetWantedLevel @ 0x00609F10`
  (`__thiscall`, ECX = ped, nivel en la pila) → `CWanted::SetWantedLevel @ 0x00562470`
- **NO escribir `+0x2C` a mano:** el nivel se deriva del caos (`+0x00`) al llamar
  a `0x00562470`; el HUD y la IA leen ambos.
- **Confianza:** ALTA en estructura y firmas (ver `docs/evidence/wanted.md`);
  MEDIA hasta prueba en juego
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

## Grupo 6 — Actores / peds (investigado, sin implementar)

Evidencia completa: [`docs/evidence/actors.md`](evidence/actors.md).

| Nativo propuesto | Mecanismo verificado | Confianza |
|---|---|---|
| `request_model(id)` | `0x0247` → `CStreaming::RequestModel @ 0x4087E0` con flags `0xC` | ALTA (binario) |
| `has_model_loaded(id)` | `0x0248` → `[0x8E4CD0 + id*0x14] == 1` | ALTA |
| `load_all_models()` | `0x0249` | ALTA |
| `create_actor(tipo, modelo, x, y, z)` | `0x009A` (caso `0x36`, bloque 100): `CPool::New @ 0x5E45E0` + ctor `0x5DDB70`/`0x5DDC60`/`0x5DE340`, tarea standstill, `SetCreatedBy(2)`, `CWorld::Add @ 0x563220` | ALTA (decompilado) |
| `delete_actor(handle)` | `0x009B` → `CPool<CPed>::GetAt @ 0x404910` + `0x486240` | ALTA |
| `get_actor_coords(handle)` | `0x00A0` (vehículo `+0x58C` / entidad `+0x14` / ped `+0x04`) | ALTA |
| `set_actor_coords(handle, x, y, z)` | `0x00A1` → `0x464DC0` | ALTA |
| `ped_handle_to_ptr(h)` | `CPools::GetPed @ 0x54FF90` (cdecl) | ALTA |
| `ped_ptr_to_handle(p)` | `CPools::GetPedRef @ 0x54FF60` (cdecl) | ALTA |

Hechos clave:

- Handle de ped = `(índice << 8) | contador`; el contador vive en el byte de
  flags del pool (`CPool<CPed>` en `0xB74490`, stride `0x7C4`).
- `CREATE_CHAR` **no pide ni carga** el modelo y **suma `+1.0f` a `z`** siempre.
- El ctor depende de `pedType` (`6` → `CCopPed`), no del modelo.
- El pool no valida `pedType` fuera de rango.

---

## Grupo 7 — Blips / iconos del radar (investigado, sin implementar)

Evidencia completa: [`docs/evidence/blips.md`](evidence/blips.md).

| Nativo propuesto | Mecanismo verificado | Confianza |
|---|---|---|
| `add_blip(x, y, z)` | `0x018A` → `SetCoordBlip(4,…)` + escala 3 | ALTA |
| `add_blip_sprite(x, y, z, sprite)` | `0x02A8` → `SetCoordBlip(4,…)` + `SetBlipSprite` | ALTA |
| `add_blip_contact(x, y, z, sprite)` | `0x02A7` → `SetCoordBlip(5,…)` + sprite | ALTA |
| `add_blip_car(coche)` | `0x0186` → `SetEntityBlip(1, coche, 0, 3)` | ALTA |
| `add_blip_actor(ped)` | `0x0187` → `SetEntityBlip(2, ped, 1, 3)` | ALTA |
| `add_blip_object(obj)` | `0x0188` → `SetEntityBlip(3, obj, 6, 3)` | ALTA |
| `set_blip_colour(b, c)` | `CRadar::ChangeBlipColour @ 0x583AB0` | ALTA |
| `set_blip_scale(b, s)` | `CRadar::ChangeBlipScale @ 0x583CC0` | ALTA |
| `set_blip_sprite(b, s)` | `CRadar::SetBlipSprite @ 0x583D70` | ALTA |
| `set_blip_display(b, d)` | `CRadar::ChangeBlipDisplay @ 0x583D20` | ALTA |
| `remove_blip(b)` | `CRadar::ClearBlip @ 0x587CE0` → `ClearActualBlip @ 0x587C10` | ALTA |

Hechos clave:

- Array `0x00BA86F0`, **175** slots, stride `0x28`; handle =
  `(contador << 16) | índice`; `0xFFFFFFFF` = inválido.
- `SetCoordBlip`/`SetEntityBlip` **ignoran el color**: coordenadas = 8,
  entidades = 7 (coche/ped) o 1. El color se cambia con `ChangeBlipColour`.
- `m_bInUse` es el bit 1 de `+0x25` (no el bit 2).
- Al llenarse (175), las funciones devuelven `0xFFFFFFFF` sin avisar.
- `0x0162`, `0x0166` y `0x0189` son **stubs** en SA 1.0.

---

## Grupo 8 — Marcadores y coronas (investigado, sin implementar)

Evidencia completa: [`docs/evidence/markers.md`](evidence/markers.md).

| Nativo propuesto | Mecanismo verificado | Confianza |
|---|---|---|
| `marker_icon(x, y, z, sprite)` | `0x04CE` → `SetShortRangeCoordBlip(4,…)` + `SetBlipSprite` | ALTA |
| `marker_asset(x, y, z, sprite)` | `0x0570` → `SetShortRangeCoordBlip(5,…)` + `SetBlipSprite` | ALTA |
| `corona_create(x,y,z,radio,tipo,flare,r,g,b)` | `0x024F` → `CCoronas::RegisterCorona` (`0x6FC580`) | ALTA |
| `corona_draw(...)` | `0x04D5` → id estable por punto de llamada | ALTA |

Hechos clave:

- `CCoronas`: **64** slots en `0xC3E058` (stride `0x3C`), contador en
  `0xC3E038`, tabla de texturas `gpCoronaTexture` en `0xC3E000`.
- El thunk `0x6FC580` indexa `gpCoronaTexture[tipo]` **sin validar el rango**:
  tipo ≥ 9 ⇒ puntero inválido ⇒ crash al renderizar.
- `0x024F` deriva el id de `script + m_pCurrentIP` **más un hash de posición**;
  `0x04D5` solo de `script + m_pCurrentIP` (apto para llamar cada frame).
- `0x04CE`/`0x0570` son **blips**, no geometría: no crean un marcador sólido.

---

## Grupo 9 — Nivel de búsqueda / wanted (investigado, sin implementar)

Evidencia completa: [`docs/evidence/wanted.md`](evidence/wanted.md).

| Nativo propuesto | Mecanismo verificado | Confianza |
|---|---|---|
| `player.wanted_level` (r/w) | cadena `Players[0].m_pPed` → `+0x480` → `+0x00` → `+0x2C`; escritura con `0x609F10` | ALTA |
| `player.wanted_no_drop` | `0x010E` → `0x609F30` → `0x562570` | ALTA |
| `player.clear_wanted()` | `0x0110` → `0x609F10(ped, 0)` | ALTA |
| `player.is_wanted(n)` | `0x010F` → compara `n < CWanted->m_nWantedLevel` | ALTA |
| `player.max_wanted` (r/w) | `0x050F` lee `0x8CDEE4`; `0x01F0` → `0x561E70` | ALTA |
| `player.wanted_multiplier` | `0x03C7` → `CWanted::m_fMultiplier` (`+0x14`) | ALTA |

Hechos clave:

- `m_nWantedLevel` (`+0x2C`) **se deriva del caos** (`+0x00`) en `0x561C90`;
  `SetWantedLevel` escribe el caos, no el nivel.
- Umbrales de caos: `50 / 180 / 550 / 1200 / 2400 / 4600` → estrellas 1..6.
- Nivel máximo inicial = **6** (`0x8CDEE4`); `0x01F0` con valor > 6 se ignora.
- El primer parámetro de `0x010D`/`0x0110`/`0x01C0` es un **índice de jugador
  (0/1)**, no un handle de ped.
- `0x0395` es *clear area*, **no** “clear wanted”.

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
| player.wanted r/w   | `0xB7CD98→+0x480→+0x2C` | ALTA (doc) | No |
| peds (crear/borrar) | `0x009A` / `0x009B` | ALTA (doc) | No |
| blips (crear/gestionar) | `0x583820…0x587CE0` | ALTA (doc) | No |
| marcadores (iconos) | `0x04CE` / `0x0570` | ALTA (doc) | No |
| coronas             | `0x6FC580` / `0x6FC180` | ALTA (doc) | No |
| give_weapon         | desconocido      | BAJA ❌   | No     |

*cheat_spawn_rhino activo pero pendiente prueba en juego

Los grupos 6–9 están **documentados y verificados en binario** pero **no
implementados**: `docs/evidence/actors.md`, `blips.md`, `markers.md` y
`wanted.md` contienen la firma, los xrefs y la secuencia exacta de cada uno.
