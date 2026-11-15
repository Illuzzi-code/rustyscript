# Blips del radar: estructura, API y opcodes

| Campo | Valor |
|---|---|
| Objetivo | Añadir, cambiar y quitar blips (iconos del radar/mapa) |
| Opcodes SCM | `0x0161`, `0x0164`–`0x0168`, `0x0186`–`0x018B`, `0x02A7`, `0x02A8`, `0x03DC` |
| Handlers | Bloque 300 (`0x0047C100`) y bloque 600 (`0x0047F370`) |
| Array de blips | `CRadar::ms_RadarTrace` en `0x00BA86F0`, 175 slots, stride `0x28` |
| Confianza | **ALTA** en decompilado y firmas; **MEDIA** hasta prueba en juego |
| Binario | GTA SA 1.0 US, SHA-256 `F01A00CE…9343AC` |

---

## 1. `tRadarTrace`: un slot del array

`CRadar::ms_RadarTrace` es un array estático de **175** entradas
(`0..174`, máximo válido `0xAE`) y **`0x28` bytes por entrada** (10 dwords).
Base verificada: `0x00BA86F0` (xrefs en `0x5838B2`, `0x583A07`, `0x585AA7`…).

| Offset | Campo | Tipo | Notas |
|---|---|---|---|
| `+0x00` | `m_nColour` | `u32` | índice de `eBlipColour` |
| `+0x04` | `m_nEntityHandle` | `u32` | handle de pool de la entidad, o índice de pickup |
| `+0x08` | `m_vecPos` | `CVector` | `x`, `y`, `z` |
| `+0x14` | `m_nCounter` | `u16` | contador de generación del handle |
| `+0x18` | `m_fSphereRadius` | `f32` | radio de la esfera (blips de tipo contacto) |
| `+0x1C` | `m_nBlipSize` | `u16` | escala del icono |
| `+0x20` | `m_pEntryExit` | `void*` | enex asociado (normalmente `0`) |
| `+0x24` | `m_nRadarSprite` | `u8` | índice de `eRadarSprite` |
| `+0x25` | flags A | `u8` | ver tabla de bits |
| `+0x26` | flags B | `u8` | ver tabla de bits |

### Bits de flags

| Byte | Bit | Campo | Significado |
|---|---|---|---|
| `+0x25` | 0 | `m_bBright` | colores brillantes (siempre 1 en blips de script) |
| `+0x25` | 1 | `m_bInUse` | **slot ocupado** |
| `+0x25` | 2 | `m_bShortRange` | no permanente en el radar |
| `+0x25` | 3 | `m_bFriendly` | afecta a `BLIP_COLOUR_THREAT` |
| `+0x25` | 4 | `m_bBlipRemain` | sobrevive a la entidad |
| `+0x25` | 5 | `m_bBlipFade` | sin uso |
| `+0x25` | 6–7 | `m_nCoordBlipAppearance` | `0` amigo, `1` amenaza |
| `+0x26` | 0–1 | `m_nBlipDisplay` | `0` ninguno, `1` solo marcador, `2` solo icono, `3` ambos |
| `+0x26` | 2–5 | `m_nBlipType` | `eBlipType` |
| `+0x26` | 6–7 | padding | se conserva al limpiar |

### Handle de blip

Los blips **no** usan el handle de `CPool`. El handle es:

```
handle = (m_nCounter << 16) | índice_de_slot      // índice en 0..174
```

- `0xFFFFFFFF` es inválido.
- Todas las operaciones de cambio comprueban **dos cosas**: que el contador del
  handle coincida con `m_nCounter` y que `m_bInUse` esté puesto. Si no, no hacen
  nada (no crashean).
- El contador se reinicia a 1 al llegar a `0xFFFE`.

---

## 2. API del motor (firmas verificadas)

| VA | Nombre | Firma real |
|---|---|---|
| `0x00583820` | `CRadar::SetCoordBlip` | `int SetCoordBlip(eBlipType type, float x, float y, float z, u32 ignored, u32 display, const char* name)` |
| `0x00583920` | `CRadar::SetShortRangeCoordBlip` | igual, y además marca `m_bShortRange` |
| `0x005839A0` | `CRadar::SetEntityBlip` | `int SetEntityBlip(eBlipType type, u32 entityHandle, u32 ignored, u32 display)` |
| `0x00583AB0` | `CRadar::ChangeBlipColour` | `void(int handle, u32 colour)` |
| `0x00583CC0` | `CRadar::ChangeBlipScale` | `void(int handle, u16 size)` |
| `0x00583D20` | `CRadar::ChangeBlipDisplay` | `void(int handle, u32 display)` |
| `0x00583D70` | `CRadar::SetBlipSprite` | `void(int handle, u8 sprite)` |
| `0x00587CE0` | `CRadar::ClearBlip` | `void(int handle)` |
| `0x00587C10` | `CRadar::ClearActualBlip` | `void(int slotIndex)` (interno) |

Notas verificadas:

- La convención de las funciones de cambio es `__cdecl`: el handle va **primero**
  en la pila (`MOV ECX,[ESP+4]` no; se lee `[ESP+4]`), y devuelven un `bool`
  implícito (el handler hace `return uVar & 0xffffff00`).
- `SetCoordBlip`/`SetEntityBlip` **ignoran el argumento de color**. El color real
  se fija con `ChangeBlipColour`.
- `ChangeBlipScale` fuerza escala `1` si `DAT_00BA67A1 != 0` (modo “radar grande”
  de misión); en caso normal escribe el valor pedido.

---

## 3. `SetCoordBlip` (`0x00583820`)

Decompilado completo (243 bytes, 8 xrefs):

```c
uint FUN_00583820(byte type, float x, float y, float z, u32 ignored, byte display)
{
  uint slot = 0;
  if ((flagsA_of_slot0 & 2) != 0) {          // si el slot 0 está ocupado, buscar otro
    uint byteOff = 0;
    do {
      if (byteOff > 6999) return 0xffffffff; // 7000/0x28 = 175 slots
      byteOff += 0x28;
      slot++;
    } while ((*(byte *)(0xba873d + byteOff) & 2) != 0);  // 0xBA873D = slot1.flagsA
    if (slot > 0xae) return 0xffffffff;
  }

  slot_pos = { x, y, z };
  flagsB(slot) = (type & 0xf) << 2 | (old & 0xc0) | (display & 3);
  counter = counter(slot);
  colour(slot)   = 8;        // BLIP_COLOUR_DESTINATION
  radius(slot)   = 1.0f;
  entity(slot)   = 0;
  size(slot)     = 1;
  sprite(slot)   = 0;
  flagsA(slot)   = 3;        // bright | inUse
  entryExit(slot)= 0;

  if (counter < 0xfffe) {
    counter(slot) = counter + 1;
    return (counter << 16) | slot;
  }
  counter(slot) = 1;
  return slot | 0x10000;
}
```

Puntos clave:

- **Color por defecto = 8** (destino). Si el script quiere otro color, debe llamar
  a `ChangeBlipColour` después.
- **Escala = 1**, sprite = 0.
- **No copia ni guarda el `scriptName`** en el slot.
- **El slot se reutiliza** sin limpiar el color anterior, pero como siempre escribe
  `8`, el resultado es determinista.
- `m_bBright` se pone a 1 siempre (`flagsA = 3`).

`SetShortRangeCoordBlip` (`0x00583920`, 258 bytes) llama a `SetCoordBlip` y, si el
handle es válido, hace `flagsA |= 4` (`m_bShortRange`). Es la base de los iconos
temporales (checkpoint, marcador de misión, etc.).

---

## 4. `SetEntityBlip` (`0x005839A0`)

```c
uint FUN_005839a0(int type, u32 entityHandle, u32 ignored, byte display)
{
  // mismo escaneo de slot libre que SetCoordBlip
  flagsB = (flagsB & ~0x3c) | ((type & 0xf) << 2);
  colour = (type == 1 || type == 2) ? 7 : 1;   // 7 = amenaza, 1 = verde
  entity = entityHandle;
  radius = 1.0f;
  size   = 1;
  sprite = 0;
  flagsB = (flagsB & ~3) | (display & 3);
  flagsA = 3;
  entryExit = 0;
  // contador y handle igual que SetCoordBlip
}
```

- El blip **sigue a la entidad**: el radar usa `m_nEntityHandle` para obtener la
  posición actual; no se guarda `m_vecPos`.
- Colores forzados: **7** (`THREAT`) para coches (`type 1`) y personajes
  (`type 2`); **1** (verde) para el resto.
- `0x03DC` (`ADD_BLIP_FOR_PICKUP`) usa `type 7` y pasa como “entidad” el índice
  de pickup devuelto por `0x1564BB0` (valida el handle del pickup y devuelve su
  índice de pool). Después aplica `ChangeBlipScale(handle, 3)`.

---

## 5. Funciones de cambio

Todas comprueban `handle != -1`, `counter == m_nCounter` e `m_bInUse`:

| Función | Campo que escribe |
|---|---|
| `ChangeBlipColour` | `+0x00` (`m_nColour`) |
| `ChangeBlipScale` | `+0x1C` (`m_nBlipSize`, `u16`) |
| `ChangeBlipDisplay` | bits 0–1 de `+0x26` |
| `SetBlipSprite` | `+0x24` (`u8`) |
| `ClearBlip` | llama a `ClearActualBlip(slot)` |

`ChangeBlipBrightness` (`0x00582870`) existe en el motor, pero el **opcode
`0x0166` es un stub** en SA 1.0 (el handler agrupa `0x0162`, `0x0166`, `0x0181`
y `0x0189` en un `break`).

---

## 6. `ClearBlip` / `ClearActualBlip`

```c
void ClearActualBlip(int slot)   // 0x00587C10, slot en 0..174
{
  size(slot)     = 1;
  flagsA(slot)   = 1;            // bright sí, inUse no
  radius(slot)   = 1.0f;
  entity(slot)   = 0;
  sprite(slot)   = 0;
  flagsB(slot)  &= 0xc0;         // borra display y type, conserva padding
}
```

**No** borra `m_nColour`, `m_nCounter` ni `m_vecPos`. Por eso el handle antiguo
queda inválido (el contador se conserva, pero `inUse` está a 0) y el slot puede
reutilizarse.

---

## 7. Opcodes SCM verificados

| Opcode | Nombre | Parámetros | Implementación verificada |
|---|---|---|---|
| `0x0161` | add blip with properties to car | `(car, arg2, display) -> var` | `SetEntityBlip(1, car, arg2, display)` |
| `0x0162` | add blip with properties to char | — | **stub** (no-op) en SA 1.0 |
| `0x0163` | add blip with properties to object | — | no presente en el bloque 300 |
| `0x0164` | remove blip | `(blip)` | `ClearBlip` |
| `0x0165` | change blip colour | `(blip, colour)` | `ChangeBlipColour` |
| `0x0166` | change blip brightness | — | **stub** (no-op) |
| `0x0167` | add blip for coord with properties | `(x, y, z, arg2, display) -> var` | `SetCoordBlip(4, x, y, z, arg2, display)` |
| `0x0168` | change blip scale | `(blip, size)` | `ChangeBlipScale` |
| `0x0186` | add blip for car | `(car) -> var` | `SetEntityBlip(1, car, 0, 3)` + `ChangeBlipScale(3)` |
| `0x0187` | add blip for char | `(char) -> var` | `SetEntityBlip(2, char, 1, 3)` + `ChangeBlipScale(3)` |
| `0x0188` | add blip for object | `(object) -> var` | `SetEntityBlip(3, object, 6, 3)` + `ChangeBlipScale(3)` |
| `0x0189` | add blip with sphere to coord | — | **stub** (no-op) |
| `0x018A` | add blip for coordinate | `(x, y, z) -> var` | `SetCoordBlip(4, x, y, z, 5, 3)` + `ChangeBlipScale(3)` |
| `0x018B` | change blip display | `(blip, display)` | `ChangeBlipDisplay` |
| `0x02A7` | add sprite blip for contact point | `(x, y, z, sprite) -> var` | `SetCoordBlip(5, x, y, z, 2, 3)` + `SetBlipSprite(sprite)` |
| `0x02A8` | add sprite blip for coord | `(x, y, z, sprite) -> var` | `SetCoordBlip(4, x, y, z, 5, 3)` + `SetBlipSprite(sprite)` |
| `0x03DC` | add blip for pickup | `(pickup) -> var` | `SetEntityBlip(7, pickupIndex, 6, 3)` + `ChangeBlipScale(3)` |

Detalle de los `arg2` ignorados: no afectan al blip; se conservan en la firma por
compatibilidad con el SCM.

---

## 8. Enumeraciones

### `eBlipType`

```
0 NONE · 1 CAR · 2 CHAR · 3 OBJECT · 4 COORD (checkpoint)
5 CONTACTPOINT (esfera) · 6 SPOTLIGHT · 7 PICKUP · 8 AIRSTRIP
```

### `eBlipDisplay`

```
0 NEITHER · 1 MARKER_ONLY · 2 BLIP_ONLY · 3 BOTH
```

### `eBlipColour`

```
0 ROJO · 1 VERDE · 2 AZUL · 3 BLANCO · 4 AMARILLO
5 ROJO (copia) · 6 AZUL (copia) · 7 THREAT · 8 DESTINATION (por defecto)
```

### `eRadarSprite` (iconos)

Rango válido: `1..63` (`RADAR_SPRITE_COUNT`); `0` = sin icono.

```
 1 WHITE          2 CENTRE         3 MAP_HERE       4 NORTH
 5 AIRYARD        6 AMMUGUN        7 BARBERS        8 BIGSMOKE
 9 BOATYARD      10 BURGERSHOT    11 BULLDOZER     12 CATALINAPINK
13 CESARVIAPANDO 14 CHICKEN       15 CJ            16 CRASH1
17 DINER         18 EMMETGUN      19 ENEMYATTACK   20 FIRE
21 GIRLFRIEND    22 HOSTPITAL     23 LOGOSYNDICATE 24 MADDOG
25 MAFIACASINO   26 MCSTRAP       27 MODGARAGE     28 OGLOC
29 PIZZA         30 POLICE        31 PROPERTYG     32 PROPERTYR
33 RACE          34 RYDER         35 SAVEGAME      36 SCHOOL
37 QMARK         38 SWEET         39 TATTOO        40 THETRUTH
41 WAYPOINT      42 TORENORANCH   43 TRIADS        44 TRIADSCASINO
45 TSHIRT        46 WOOZIE        47 ZERO          48 DATEDISCO
49 DATEDRINK     50 DATEFOOD      51 TRUCK         52 CASH
53 FLAG          54 GYM           55 IMPOUND       56 LIGHT
57 RUNWAY        58 GANGB         59 GANGP         60 GANGY
61 GANGN         62 GANGG         63 SPRAY
```

---

## 9. Límites y trampas

1. **Solo 175 blips simultáneos**. Al llenarse, `SetCoordBlip`/`SetEntityBlip`
   devuelven `0xFFFFFFFF` y el handler escribe ese valor en la variable de salida.
2. **El color no se pasa** a `Set*Blip`; hay que llamar siempre a
   `ChangeBlipColour` si se quiere un color distinto del 8 (coordenadas) o del
   7/1 (entidades).
3. **Los blips de entidad no tienen posición propia**: si la entidad se borra, el
   blip puede quedar apuntando a un handle reutilizado. Usar `ClearBlip` antes de
   borrar la entidad, o marcar `m_bBlipRemain`.
4. **`0x0162`, `0x0166`, `0x0189` no hacen nada** en SA 1.0. Para un blip de
   personaje con propiedades, usar `0x0187` y luego `0x0165`/`0x0168`.
5. **Guardar el handle** devuelto por el SCM; no reconstruirlo a mano. El contador
   cambia cada vez que se reutiliza el slot.
6. **`0x03DC` espera el handle del pickup**, no un handle de entidad; el motor lo
   convierte con `0x1564BB0`.

---

## 10. Firmas verificadas

| VA | Símbolo | Prólogo (16 bytes) | Xrefs |
|---|---|---|---|
| `0x00583820` | `SetCoordBlip` | `A0 15 87 BA 00 53 56 B1 02 33 D2 33 F6 84 C1 74` | 8 |
| `0x00583920` | `SetShortRangeCoordBlip` | `8B 44 24 1C 8B 4C 24 18 8B 54 24 14 56 50 51 8B` | 2 |
| `0x005839A0` | `SetEntityBlip` | `A0 15 87 BA 00 56 33 F6 A8 02 74 2A 33 C0 8B FF` | 10 |
| `0x00583AB0` | `ChangeBlipColour` | `8B 4C 24 04 83 F9 FF 74 31 8B C1 25 FF FF 00 00` | 6 |
| `0x00583CC0` | `ChangeBlipScale` | `8B 4C 24 04 83 F9 FF 74 22 8B C1 25 FF FF 00 00` | 11 |
| `0x00583D20` | `ChangeBlipDisplay` | `8B 54 24 04 83 FA FF 74 41 8B C2 25 FF FF 00 00` | 3 |
| `0x00583D70` | `SetBlipSprite` | `8B 4C 24 04 83 F9 FF 74 31 8B C1 25 FF FF 00 00` | 5 |
| `0x00587CE0` | `ClearBlip` | `8B 4C 24 04 83 F9 FF 74 30 8B C1 25 FF FF 00 00` | 5 |
| `0x00587C10` | `ClearActualBlip` | `8B 44 24 04 33 D2 3B C2 7C 44 3D AF 00 00 00 7D` | 1 |
| `0x01564BB0` | handle de pickup → índice | `8B 4C 24 04 83 F9 FF 74 18 8B C1 25 FF FF 00 00` | 0 |

---

## 11. API propuesta para RustyScript

```rscript
# Iconos del radar
var b = add_blip(x, y, z)                 # 0x018A equivalente
var b = add_blip_sprite(x, y, z, sprite)  # 0x02A8
var b = add_blip_contact(x, y, z)         # 0x02A7
var b = add_blip_car(coche)               # 0x0186
var b = add_blip_char(actor)              # 0x0187
set_blip_colour(b, 4)                     # amarillo
set_blip_scale(b, 2)
set_blip_sprite(b, 41)                    # waypoint
set_blip_display(b, 3)
remove_blip(b)
```

Mientras no exista una VM de handles de actores/vehículos, los blips de entidad
deberían exponerse solo cuando el script ya tenga un handle válido del motor.
