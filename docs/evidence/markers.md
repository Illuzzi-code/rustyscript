# Marcadores, iconos y coronas

| Campo | Valor |
|---|---|
| Objetivo | Crear marcadores de misión (iconos del radar) y coronas visibles en el mundo |
| Opcodes SCM | `0x04CE` icon marker, `0x0570` asset radar marker, `0x024F` create corona, `0x04D5` draw corona |
| Opcodes de blip relacionados | `0x018A`, `0x02A7`, `0x02A8` (ver [`blips.md`](blips.md)) |
| Handlers | Bloque 1200 (`0x0048B590`) para `0x04CE`/`0x04D5`, bloque 1300 (`0x0048CDD0`) para `0x0570`, bloque 500 (`0x0047E090`) para `0x024F` |
| Confianza | **ALTA** en decompilado y firmas; **MEDIA** hasta prueba en juego |
| Binario | GTA SA 1.0 US, SHA-256 `F01A00CE…9343AC` |

> Convención de este documento: “marcador” = icono de radar creado con un blip
> (tipos 4 y 5 de `eBlipType`), “corona” = sprite luminoso 3D dibujado por
> `CCoronas`. El SCM llama “marker” a las dos cosas y eso confunde; aquí se
> separan porque usan motores distintos.

---

## 1. Qué crea cada opcode

| Opcode | Nombre SCM | Motor real | Efecto visible |
|---|---|---|---|
| `0x04CE` | create icon marker | `CRadar::SetShortRangeCoordBlip` + `CRadar::SetBlipSprite` | icono en el radar y marcador en el mundo, con sprite elegible y alcance corto |
| `0x0570` | create asset radar marker with icon | `CRadar::SetShortRangeCoordBlip` (tipo 5) + `SetBlipSprite` | igual que `0x04CE` pero con tipo *contact point* (esfera) |
| `0x024F` | create corona | `CCoronas::RegisterCorona` (variante por tipo) | corona 3D en la posición, con radio, tipo y *flare* |
| `0x04D5` | draw corona | `CCoronas::RegisterCorona` (variante por tipo) | igual, pero con id estable por punto de llamada → se puede llamar cada frame |
| `0x018A` | add blip for coord | `CRadar::SetCoordBlip(4,…)` + escala 3 | icono de destino “normal” (sin sprite elegible) |
| `0x02A7` | add sprite blip for contact point | `CRadar::SetCoordBlip(5,…)` + sprite | igual que `0x0570` sin `m_bShortRange` |
| `0x02A8` | add sprite blip for coord | `CRadar::SetCoordBlip(4,…)` + sprite | igual que `0x04CE` sin `m_bShortRange` |

Las diferencias entre `0x04CE`/`0x02A8` y entre `0x0570`/`0x02A7` son solo el flag
`m_bShortRange` (`+0x25` bit 2) y que `0x04CE`/`0x0570` no llaman a
`ChangeBlipScale` (dejan `m_nBlipSize = 1`).

---

## 2. `0x04CE` — create icon marker

Handler: bloque 1200, caso `0x4CE` (VA `0x0048BB3E`).

### Parámetros (4)

| # | Tipo | Significado |
|---|---|---|
| 1 | `float` | `x` |
| 2 | `float` | `y` |
| 3 | `float` | `z` (`< -100.0` ⇒ calcular suelo) |
| 4 | `int` | `sprite` (`eRadarSprite`, `1..63`; `0` = sin icono) |

Escribe el handle del blip en la variable de salida del SCM.

### Decompilado

```c
case 0x4ce:
  FUN_00464080(4);                                   // leer 4 parámetros
  if (z < -100.0f)  z = FUN_00569660(x, y);          // CWorld::FindGroundZ
  FUN_00582870(FUN_00464250());                      // (no-op real: ver blips.md)
  handle = FUN_00583920(4, x, y, z, 5, 3, scriptName); // SetShortRangeCoordBlip
  FUN_00583d70(handle, sprite);                      // SetBlipSprite
  escribir_salida(handle);
```

Notas verificadas:

- El tipo es **4** (`BLIP_COORD`, checkpoint) y el `display` es **3** (icono y
  marcador), igual que `0x018A`.
- La diferencia con `0x018A` es que `SetShortRangeCoordBlip` marca
  `m_bShortRange` (`+0x25` bit 2) y que aquí se aplica un sprite arbitrario.
- El `scriptName` (`script + 8`) se pasa pero `SetCoordBlip` **no lo guarda**.
- No se llama a `ChangeBlipScale`: el tamaño queda en `1` (los blips “grandes”
  del SCM usan escala `3`).

---

## 3. `0x0570` — create asset radar marker with icon

Handler: bloque 1300, caso `0x570` (VA `0x0048D686`).

### Parámetros

El handler llama a `FUN_00464080(4)`, es decir **lee 4 parámetros numéricos**, en
este orden:

| # | Tipo real | Significado |
|---|---|---|
| 1 | `float` | `x` |
| 2 | `float` | `y` |
| 3 | `float` | `z` (`< -100.0` ⇒ calcular suelo) |
| 4 | `int` | `sprite` |

> ⚠️ La tabla pública de opcodes describe `0x0570` como
> `<int, flt, flt, flt, var>` (“parámetros reordenados en Sanny Builder”), pero
> el binario usa el primer parámetro como `x` de la posición. Se documenta lo
> que hace el ejecutable; **hay que probarlo en juego** antes de dar la firma
> por buena.

### Decompilado

```c
case 0x570:
  FUN_00464080(4);
  if (z < -100.0f) z = FUN_00569660(x, y);
  FUN_00582870(FUN_00464250());
  handle = FUN_00583920(5, x, y, z, 2, 3, scriptName);   // tipo 5 = CONTACTPOINT
  FUN_00583d70(handle, sprite);                         // SetBlipSprite
  escribir_salida(handle);
  return;
```

Diferencia con `0x04CE`: tipo **5** (`BLIP_CONTACTPOINT`, se dibuja como esfera
usando `m_fSphereRadius`) y el argumento ignorado es `2` en vez de `5`. En ambos
casos `m_fSphereRadius` queda en `1.0f` porque `SetCoordBlip` la inicializa así.

---

## 4. Coronas: `CCoronas`

### 4.1 Estructura `CRegisteredCorona`

Array estático de **64** entradas (`MAX_NUM_CORONAS`), base **`0x00C3E058`**,
stride **`0x3C`**. Contador de coronas registradas en el frame:
`NumCoronas` = **`0x00C3E038`**.

| Offset | Campo | Tipo |
|---|---|---|
| `+0x00` | `m_vPosn` | `CVector` |
| `+0x0C` | `m_dwId` | `u32` |
| `+0x10` | `m_pTexture` | `RwTexture*` |
| `+0x14` | `m_fSize` | `f32` |
| `+0x18` | `m_fAngle` | `f32` |
| `+0x1C` | `m_fFarClip` | `f32` |
| `+0x20` | `m_fNearClip` | `f32` |
| `+0x24` | `m_fHeightAboveGround` | `f32` |
| `+0x28` | `m_fFadeSpeed` | `f32` |
| `+0x2C` | `m_Color` | `CRGBA` |
| `+0x30` | `m_nFadeState` | `u8` |
| `+0x31` | `m_bRegisteredThisFrame` | `bool` |
| `+0x32` | `m_nFlareType` | `u8` |
| `+0x33` | `m_bUsesReflection` | `bool` |
| `+0x35` | `m_bJustCreated` | `bool` |
| `+0x38` | `m_pAttachedTo` | `CEntity*` |

El motor busca la corona por `m_dwId`; si no existe y hay hueco, ocupa una libre.
Si las 64 están ocupadas y la llamada no es “replace”, **no hace nada**.

### 4.2 `CCoronas::RegisterCorona`

Hay dos sobrecargas. La que usa el SCM (`0x024F`/`0x04D5`) es la **variante por
tipo**, que es un thunk de 16 bytes en **`0x006FC580`**:

```
0F B6 54 24 28   movzx edx, byte ptr [esp+0x28]   ; arg 10 = tipo de corona
8B 04 95 00E0C300 mov eax, dword ptr [edx*4+0x00C3E000]  ; gpCoronaTexture[tipo]
89 44 24 28      mov dword ptr [esp+0x28], eax    ; sustituye el tipo por la textura
E9 EB FB FF FF   jmp 0x006FC180                   ; RegisterCorona(textura)
```

- `gpCoronaTexture` = **`0x00C3E000`**: tabla de `RwTexture*` indexada por tipo.
- El índice se toma como **byte sin comprobar rango**: un tipo ≥ 9 lee un puntero
  de textura inválido. Mantener el tipo en `0..8`.
- La función real es **`0x006FC180`**, 831 bytes, 21 argumentos:

```
void CCoronas::RegisterCorona(
    u32 id, CEntity* attachTo, u8 r, u8 g, u8 b, u8 a,
    CVector const& pos, float radius, float farClip, RwTexture* texture,
    eCoronaFlareType flare, bool enableReflection, bool checkObstacles,
    int unused, float angle, bool longDistance, float nearClip,
    u8 fadeState, float fadeSpeed, bool onlyFromBelow, bool reflectionDelay)
```

### 4.3 `0x024F` — create corona

Handler: bloque 500, caso `0x24F` (VA `0x0047F08E`).

Parámetros (9): `x, y, z, radius, tipo, flare, r, g, b`.

Llamada generada (orden real de los `push`):

```c
id = script + *(u32*)(script + 0x14) + hash;   // ver nota
CCoronas::RegisterCorona(
    id, 0, r, g, b, 0xFF, &pos, radius,
    450.0f /* farClip */, tipo, flare,
    true /* enableReflection */, false /* checkObstacles */, 0, 0.0f /* angle */,
    false /* longDistance */, 1.5f /* nearClip */,
    0 /* fadeState */, 15.0f /* fadeSpeed */, false, false);
```

- `script + 0x14` = `CRunningScript::m_pCurrentIP`; por tanto el id incluye la
  dirección de la instrucción actual.
- Además, el handler suma un **hash entero derivado de la posición**, calculado
  en ensamblador como `(int)((coord + 3000.0f) * 12001.0f) + (int)(coord + 3000.0f)`
  (constantes `0x00859A90 = -3000.0f` y `0x00859F1C = 12001.0f`). Consecuencia
  práctica: **la misma posición reutiliza la misma corona**, pero dos posiciones
  distintas consumen dos slots.
- `z < -100.0f` ⇒ `CWorld::FindGroundZ(x, y)` (sin el `+1.0f` que sí aplica
  `CREATE_CHAR`).
- `radius` va tal cual a `m_fSize`; `farClip` fijo 450.0; `nearClip` fijo 1.5.
- `alpha` fijo a `255`.

### 4.4 `0x04D5` — draw corona

Handler: bloque 1200, caso `0x4D5` (VA `0x0048C4CA`).

Mismos 9 parámetros y misma llamada, con dos diferencias:

```c
id = script + *(u32*)(script + 0x14);   // sin hash: id estable por punto de llamada
CCoronas::RegisterCorona(id, 0, r, g, b, 0xFF, &pos, radius,
                         150.0f, tipo, flare, true, false, 0, 0.0f, false,
                         0.2f, 0, 15.0f, false, false);
```

- Al no incluir hash, **cada punto de llamada tiene exactamente una corona**: es
  la opcode pensada para llamarse cada frame (por eso “draw” y no “create”).
- `farClip` 150.0 y `nearClip` 0.2 (más pequeña y cercana que `0x024F`).

### 4.5 Recomendación de implementación

Para una nativa propia **no conviene replicar los ids del SCM**. Es más simple
llamar a `0x006FC180` (variante por textura) con:

- `id` = contador propio por corona lógica (estable mientras el script la use),
- `texture` = `gpCoronaTexture[tipo]` leído de `0x00C3E000` (o el puntero que se
  quiera), con `tipo` validado a `0..8`,
- `attachTo = nullptr` y posición absoluta.

Llamar a `0x006FC580` directamente también es válido y evita leer la tabla a mano,
pero **no valida el tipo**.

---

## 5. Límites y trampas

1. **Blips**: máximo 175 (`0xAE`); al llenarse, `SetCoordBlip` devuelve
   `0xFFFFFFFF` y el handler lo escribe igualmente en la variable de salida.
2. **Coronas**: máximo 64. Si están todas ocupadas y el id no coincide, la
   llamada se descarta en silencio.
3. **`0x04CE`/`0x0570` no crean geometría**: son blips del radar. Un “marcador”
   visible en el mundo y persistente se consigue con una corona (`0x04D5`) o con
   un objeto, no con estas opcodes.
4. **Los blips `short range` desaparecen del radar a distancia**, pero siguen
   existiendo; para un icono permanente usar `0x02A7`/`0x02A8`.
5. **`sprite` fuera de `1..63`**: `SetBlipSprite` escribe el byte sin validar; el
   radar puede pintar basura o no pintar nada. Usar `1..63`.
6. **`tipo` de corona fuera de `0..8`**: el thunk indexa `gpCoronaTexture` sin
   comprobar rango ⇒ puntero inválido ⇒ *crash* al renderizar.
7. **Borrar el blip antes de borrar la entidad** (`0x0164`) si el blip es de
   entidad; los de coordenadas no dependen de nada.
8. Ninguno de estos opcodes requiere streaming de modelos: no llaman a
   `CStreaming`.

---

## 6. Firmas verificadas

| VA | Símbolo | Prólogo (16 bytes) | Xrefs |
|---|---|---|---|
| `0x00583920` | `CRadar::SetShortRangeCoordBlip` | `8B 44 24 1C 8B 4C 24 18 8B 54 24 14 56 50 51 8B` | 2 |
| `0x00583D70` | `CRadar::SetBlipSprite` | `8B 4C 24 04 83 F9 FF 74 31 8B C1 25 FF FF 00 00` | 5 |
| `0x006FC180` | `CCoronas::RegisterCorona` (textura) | `83 EC 1C 55 56 8B 74 24 2C 85 F6 57 74 46 8B 46` | 6 |
| `0x006FC580` | `CCoronas::RegisterCorona` (tipo) | `0F B6 54 24 28 8B 04 95 00 E0 C3 00 89 44 24 28` | 28 |
| `0x00569660` | `CWorld::FindGroundZ` | `83 EC 38 8B 44 24 3C 8B 4C 24 40 6A 00 6A 00 6A` | 64 |

Datos globales:

| VA | Símbolo | Contenido |
|---|---|---|
| `0x00C3E000` | `gpCoronaTexture` | `RwTexture*[9]` (índice = tipo de corona) |
| `0x00C3E038` | `CCoronas::NumCoronas` | `u32` coronas registradas este frame |
| `0x00C3E058` | `CCoronas::aCoronas` | `CRegisteredCorona[64]`, stride `0x3C` |

---

## 7. API propuesta para RustyScript

```rscript
# Marcadores de radar (blips con icono)
var m = marker_icon(x, y, z, 41)        # 0x04CE, sprite 41 = waypoint
var m = marker_asset(x, y, z, 41)       # 0x0570, tipo contact point
var m = blip_sprite(x, y, z, 41)        # 0x02A8 (permanente)
set_blip_colour(m, 4)
set_blip_scale(m, 3)
remove_blip(m)

# Coronas
var c = corona_create(x, y, z, 2.0, 1, 0, 255, 0, 0)   # 0x024F
corona_draw(id, x, y, z, 2.0, 1, 0, 255, 0, 0)         # 0x04D5 (cada frame)
```

`corona_draw` debería exponerse con un id propio y llamarse en cada frame desde
un evento `on_tick`; `corona_create` solo una vez.
