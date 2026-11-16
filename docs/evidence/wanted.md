# Nivel de búsqueda (wanted): estructura, opcodes y API

| Campo | Valor |
|---|---|
| Objetivo | Leer y escribir el nivel de búsqueda del jugador |
| Opcodes SCM | `0x010D` set, `0x010E` set no-drop, `0x010F` comparar, `0x0110` clear, `0x01C0` leer, `0x01F0` máximo, `0x03C7` multiplicador, `0x050F` leer máximo |
| Handlers | Bloque 200 (`0x00469390`) para `0x010D`–`0x0110`, bloque 400 (`0x0047D210`) para `0x01C0`/`0x01F0`, bloque 900 (`0x00483BD0`) para `0x03C7`, bloque 1200 (`0x0048B590`) para `0x050F` |
| Confianza | **ALTA** en decompilado, offsets y firmas; **MEDIA** hasta prueba en juego |
| Binario | GTA SA 1.0 US, SHA-256 `F01A00CE…9343AC` |

> Todos los offsets de `CPlayerInfo`, `CPlayerData` y `CWanted` se contrastaron con
> `plugin-sdk` (`VALIDATE_OFFSET`) y con el binario. Las globales y las funciones
> salen solo del ejecutable.

---

## 1. Cadena de datos

```
CWorld::Players            0x00B7CD98      CPlayerInfo[2], stride 0x190
  Players[i].m_pPed        +0x00           CPed* (siempre un CPlayerPed)
  Players[i].m_PlayerData  +0x04           CPlayerData, 0xAC bytes
    .m_pWanted             +0x00           CWanted*, 0x29C bytes
      .m_nWantedLevel      +0x2C           u32 (0..6)

CPed::m_pPlayerData        +0x480          CPlayerData* (solo peds de jugador)
CWorld::PlayerInFocus      0x00B7CD74      u8 (0 o 1)
```

`CPlayerData` está **embebido** en `CPlayerInfo` (no es un puntero), y
`CPlayerInfo` mide exactamente `0x190`, que es el stride que usa el motor.

### Helper `0x0056E230` — `Players[i].m_pWanted`

```c
CWanted* FUN_0056e230(int idx)
{
  if (idx < 0) idx = *(u8*)0x00B7CD74;          // CWorld::PlayerInFocus
  return *(CWanted**)(0x00B7CD9C + idx * 0x190);
}
```

`0x00B7CD9C` es simplemente `&Players[0].m_PlayerData`, es decir
`&Players[0].m_pWanted`. Por eso el helper devuelve el `CWanted*` con una sola
indirección.

### Helper `0x0156D300` — `CPed::GetWantedLevel`

```c
int FUN_0156d300(CPed* ped)
{
  if (ped->m_pPlayerData == 0) return 0;
  return ped->m_pPlayerData->m_pWanted->m_nWantedLevel;
}
```

---

## 2. Estructura `CWanted` (0x29C bytes)

| Offset | Campo | Tipo | Notas |
|---|---|---|---|
| `+0x00` | `m_nChaosLevel` | `u32` | **caos**, no el nivel; lo escribe `SetWantedLevel` |
| `+0x04` | `m_nChaosLevelBeforeParole` | `u32` | |
| `+0x08` | `m_nLastTimeWantedDecreased` | `u32` | |
| `+0x0C` | `m_nLastTimeWantedLevelChanged` | `u32` | lo actualiza `0x561C90` |
| `+0x10` | `m_nTimeOfParole` | `u32` | |
| `+0x14` | `m_fMultiplier` | `f32` | opcode `0x03C7` |
| `+0x18` | `m_nCopsInPursuit` | `u8` | |
| `+0x19` | `m_nMaxCopsInPursuit` | `u8` | lo fija `0x561C90` |
| `+0x1A` | `m_nMaxCopCarsInPursuit` | `u8` | lo fija `0x561C90` |
| `+0x1B` | `m_nCopsBeatingSuspect` | `u8` | |
| `+0x1C` | `m_nChanceOnRoadBlock` | `u16` | |
| `+0x1E` | flags (`m_bPoliceBackOff`, …) | bitfield | |
| `+0x20` | `m_nCurrentChaseTime` | `u32` | |
| `+0x24` | `m_nCurrentChaseTimeCounter` | `u32` | |
| `+0x28` | `m_nTimeCounting` | `u32` | |
| `+0x2C` | **`m_nWantedLevel`** | `u32` | 0..6; es el valor que se lee |
| `+0x30` | `m_nWantedLevelBeforeParole` | `u32` | |
| `+0x34` | `m_CrimesBeingQd[16]` | 16 × `0x1C` | se pone a 0 al fijar nivel |
| `+0x1F4` | `m_pCopsInPursuit` | puntero | |
| `+0x21C` | `m_PoliceScannerAudio` | objeto | |
| `+0x298` | `m_bLeavePlayerAlone` | `u8` | “la policía ignora al jugador” |

---

## 3. Cómo se calcula el nivel: caos → estrellas

`m_nWantedLevel` (`+0x2C`) **no se escribe directamente**: es una función del
caos (`+0x00`) que recalcula `CWanted::UpdateWantedLevel` (`0x00561C90`).

### `0x00562470` — `CWanted::SetWantedLevel(level)`

```c
void __thiscall SetWantedLevel(CWanted* this, int level)
{
  if (*(u8*)0x00969171 != 0) return;          // flag global: ignora todo
  if (level > *(u32*)0x008CDEE4)              // tope = nivel máximo
    level = *(u32*)0x008CDEE4;

  for (int i = 0; i < 16; i++)                // limpia delitos
    this->m_CrimesBeingQd[i].clear();         // +0x34, stride 0x1C

  switch (level) {
    case 0: this->m_nChaosLevel = 0;    break;
    case 1: this->m_nChaosLevel = 0x46;   break;   // 70
    case 2: this->m_nChaosLevel = 200;    break;
    case 3: this->m_nChaosLevel = 0x23A;  break;   // 570
    case 4: this->m_nChaosLevel = 0x4C4;  break;   // 1220
    case 5: this->m_nChaosLevel = 0x974;  break;   // 2420
    case 6: this->m_nChaosLevel = 0x120C; break;   // 4620
  }
  FUN_00561C90(this);                         // recalcula +0x2C
}
```

### `0x00561C90` — `CWanted::UpdateWantedLevel`

```c
void __fastcall FUN_00561c90(CWanted* this)
{
  if (this->m_nChaosLevel > *(u32*)0x008CDEE8)
    this->m_nChaosLevel = *(u32*)0x008CDEE8;   // tope de caos del nivel máximo

  u32 chaos = this->m_nChaosLevel;
  this->m_nWantedLevel =                    // +0x2C
      chaos <   50 ? 0 :
      chaos <  180 ? 1 :
      chaos <  550 ? 2 :
      chaos < 1200 ? 3 :
      chaos < 2400 ? 4 :
      chaos < 4600 ? 5 : 6;

  // ajusta m_nMaxCopsInPursuit, m_nMaxCopCarsInPursuit, m_nChanceOnRoadBlock
  // según el nivel (1/1/0, 2/2/0, 2/3/0, 2/4/0xC, 3/6/0x12, 3/8/0x18, 3/10/0x1E)
}
```

Umbrales de caos verificados (constantes del binario):
`50`, `180`, `550`, `1200`, `2400`, `4600`.

### `0x00562570` — `CWanted::SetWantedLevelNoDrop`

```c
if (this->m_nWantedLevel < this->m_nWantedLevelBeforeParole)
  SetWantedLevel(this, this->m_nWantedLevelBeforeParole);   // restaura tras libertad provisional
if (this->m_nWantedLevel < level)
  SetWantedLevel(this, level);                              // solo sube
```

### Globales de configuración

| VA | Símbolo | Valor inicial | Quién lo escribe |
|---|---|---|---|
| `0x008CDEE4` | nivel máximo permitido | `6` | `0x01F0` (`0x561270`) y `0x561C64` (init) |
| `0x008CDEE8` | tope de caos del nivel máximo | `0x23F0` (9200) | `0x01F0` y `0x561C64` |
| `0x00969171` | flag que bloquea cambios de nivel | `0` | `0x004396D4` (lo alterna) |

Tabla de `0x00561E70` (`SET_MAX_WANTED_LEVEL`):

| `level` | `0x8CDEE4` | `0x8CDEE8` |
|---|---|---|
| 0 | 0 | 0 |
| 1 | 1 | `0x73` (115) |
| 2 | 2 | `0x16D` (365) |
| 3 | 3 | `0x36B` (875) |
| 4 | 4 | `0x708` (1800) |
| 5 | 5 | `0xDAC` (3500) |
| 6 | 6 | `0x1AF4` (6900) |
| `>6` | — | se ignora |

> Quirk verificado: el juego inicializa el tope de caos a `9200` (`0x23F0`), pero
> `SET_MAX_WANTED_LEVEL(6)` lo baja a `6900`. Sigue siendo nivel 6 (umbral 4600),
> así que no cambia las estrellas, solo el margen antes de bajar de nivel.

Si `0x00969171` es distinto de 0, **todas** las llamadas a `SetWantedLevel` se
ignoran sin error. El juego lo alterna en `0x004396D4`; una nativa no debería
tocarlo.

---

## 4. Opcodes SCM verificados

### `0x010D` — set wanted level

Handler: bloque 200, caso `0x10D` (VA `0x004699BB`). 2 parámetros:
**índice de jugador** (`0`/`1`) y nivel.

Ensamblador real:

```
push 2
mov  ecx, esi
call FUN_00464080                  ; leer parámetros
mov  ecx, [0x00A43C78]             ; parámetro 1 = índice de jugador
mov  eax, [0x00A43C7C]             ; parámetro 2 = nivel
imul ecx, ecx, 0x190
mov  ecx, [ecx + 0x00B7CD98]       ; Players[idx].m_pPed
push eax
call 0x00609F10                    ; CPed::SetWantedLevel
```

> El primer parámetro es un **índice de jugador**, no un handle de ped: se usa
> como multiplicador de `0x190` sobre `CWorld::Players`. Pasar un handle de ped
> (p. ej. `1`) leería `Players[1]`, que puede ser nulo.

### `0x00609F10` — `CPed::SetWantedLevel`

```
mov  eax, [ecx+0x480]      ; m_pPlayerData
test eax, eax
jz   .null
mov  eax, [eax]            ; m_pWanted
mov  ecx, eax
jmp  0x00562470            ; CWanted::SetWantedLevel(this, level)
.null:
xor  eax, eax
mov  ecx, eax
jmp  0x00562470            ; ECX = 0 → crash si el ped no es de jugador
```

Consecuencia: solo se puede llamar con un ped que tenga `m_pPlayerData`.

### `0x010E` — set wanted level no-drop

Caso `0x10E` (VA `0x004699E8`). Mismo esquema, pero llama a
`0x00609F30` → `0x00562570` (`CWanted::SetWantedLevelNoDrop`). Solo **sube** el
nivel y restaura el nivel previo a la libertad provisional.

### `0x010F` — is wanted level greater than

Caso `0x10F`. 2 parámetros: índice de jugador y nivel.

```c
CWanted* w = FUN_0056e230(playerIndex);
FUN_004859d0(level < w->m_nWantedLevel);   // resultado de condición (respeta NOT)
```

> El opcode devuelve cierto si el nivel actual es **mayor** que el parámetro.

### `0x0110` — clear wanted level

Caso `0x0110`. 1 parámetro: índice de jugador.

```
push 1
call FUN_00464080
mov  edx, [0x00A43C78]
imul edx, edx, 0x190
mov  ecx, [edx + 0x00B7CD98]   ; Players[idx].m_pPed
push 0
call 0x00609F10                ; SetWantedLevel(ped, 0)
```

### `0x01C0` — store wanted level

Handler: bloque 400, caso `0x1C0` (llamada a `0x0041BE60` en `0x0047D6C3`).
1 parámetro: índice de jugador; escribe el nivel en la variable de salida.

```c
ped = Players[idx].m_pPed;                       // imul 0x190 + 0xB7CD98
out = CPed::GetWantedLevel(ped);                 // thunk 0x41BE60 -> 0x156D300
```

### `0x01F0` — set max wanted level

Caso `0x1F0`. 1 parámetro: `0..6`; valores mayores se ignoran. Llama a
`0x00561E70`, que escribe `0x8CDEE4` y `0x8CDEE8` según la tabla de §3.

### `0x03C7` — set wanted multiplier

Handler: bloque 900, caso `0x3C7` (llamada a `0x56E230` en `0x0048500E`).

```c
CWanted* w = FUN_0056e230(-1);        // jugador en foco
w->m_fMultiplier = param;             // +0x14
```

El multiplicador afecta a los **delitos futuros**, no al nivel actual.

### `0x050F` — get max wanted level

Caso `0x50F`.

```c
out = *(u32*)0x008CDEE4;              // nivel máximo actual
```

### Otros opcodes que NO son lo que parecen

- `0x0395` es **clear area** (5 parámetros: x, y, z, radio, flag), no “clear
  wanted”. Verificado en el handler del bloque 900.
- `0x0111` escribe `script->m_bWastedBustedCheck` (`+0xD3`); no toca el nivel.
- `0x0112` lee `script->m_bWastedOrBusted` (`+0xD4`) como condición.

---

## 5. Límites y trampas

1. **Solo 0..6 estrellas.** `SetWantedLevel` recorta a `0x8CDEE4` (6 por
   defecto); `UpdateWantedLevel` nunca produce más de 6.
2. **No escribir `+0x2C` a mano** salvo que se quiera un cambio “cosmético”: el
   HUD, la radio policial y la IA leen además el caos (`+0x00`) y los delitos.
   La vía correcta es `CWanted::SetWantedLevel` (`0x561870`).
3. **Usar un ped de jugador.** `0x609310`/`0x609330` saltan a `0x561870` con
   `ECX = 0` si el ped no tiene `m_pPlayerData`; eso revienta.
4. **El índice de jugador es 0/1**, no un handle de ped (ver §4).
5. **`0x01F0` con valores > 6 no hace nada** (no hay clamp: se ignora).
6. **El flag `0x00969171`** puede dejar las llamadas sin efecto; no modificarlo.
7. **Bajar de nivel** requiere `SetWantedLevel` (recorta caos); usar
   `SetWantedLevelNoDrop` si se quiere que la policía no “perdone” al jugador.
8. La lectura (`+0x2C`) es segura en cualquier momento; la escritura debe hacerse
   desde el hilo principal (el mismo donde corre el hook), no desde un callback
   de otro hilo.

---

## 6. Firmas verificadas

> **CORRECCIÓN (verificada en binario buscando firma exacta):**
> Las VAs de Ghidra tenían un offset erróneo. Las direcciones reales son:

| VA | Símbolo | Prólogo (16 bytes) | Conv | RET |
|---|---|---|---|---|
| `0x00561870` | `CWanted::SetWantedLevel` | `A0 71 91 96 00 84 C0 0F 85 96 00 00 00 A1 E4 DE` | `__thiscall` | `RET 4` (callee limpia) |
| `0x00561970` | `CWanted::SetWantedLevelNoDrop` | `56 8B F1 8B 46 30 39 46 2C 7D 08 50 8B CE E8 ED` | `__thiscall` | `RET 4` |
| `0x00561270` | `SetMaximumWantedLevel` | `8B 44 24 04 83 F8 06 0F 87 91 00 00 00 FF 24 85` | `__cdecl` | `RET` (caller limpia) |
| `0x00609310` | `CPed::SetWantedLevel` | `8B 81 80 04 00 00 85 C0 74 09 8B 00 8B C8 E9 4D` | `__thiscall` | — |
| `0x0056E230` | `Players[i].m_pWanted` | `8B 44 24 04 85 C0 7D 07 0F B6 05 74 CD B7 00 69` | `__cdecl` | — |

Globales:

| VA | Símbolo |
|---|---|
| `0x00B7CD98` | `CWorld::Players` |
| `0x00B7CD74` | `CWorld::PlayerInFocus` |
| `0x008CDEE4` | nivel máximo de búsqueda |
| `0x008CDEE8` | tope de caos |
| `0x00969171` | flag de bloqueo de cambios |

---

## 7. API propuesta para RustyScript

```rscript
player.wanted_level          # lectura: 0..6 (cadena Players[0].m_pPed ->
                             #   +0x480 -> +0x00 -> +0x2C)
player.wanted_level = 3      # escritura: CPed::SetWantedLevel(ped, clamp(0..max))
player.wanted_no_drop = 3    # 0x010E: solo sube
player.clear_wanted()        # 0x0110
player.max_wanted            # 0x050F
player.max_wanted = 4        # 0x01F0 (0..6; >6 se ignora)
player.wanted_multiplier     # 0x03C7, float
player.is_wanted(2)          # 0x010F: nivel actual > 2
```

Implementación mínima y segura:

1. `ped = *(void**)(0x00B7CD98)` (jugador 0) — comprobar que no es nulo.
2. Lectura: `*(u32*)(*(u32*)(ped + 0x480) + 0x2C)`.
3. Escritura: `CPed::SetWantedLevel` (`0x00609F10`, `__thiscall` con `ECX = ped`)
   y nivel en la pila; **no** escribir `+0x2C` directamente.
4. Para el máximo, `0x00561E70` (`__cdecl`, 1 argumento).

Ninguna de estas funciones requiere streaming ni toca el pool de peds.
