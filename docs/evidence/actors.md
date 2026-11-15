# Actores (peds): crear, borrar y consultar

| Campo | Valor |
|---|---|
| Objetivo | Crear y manipular actores (peds) desde script |
| Opcodes SCM implicados | `0x009A` create char, `0x009B` delete char, `0x00A0` get char coords, `0x00A1` set char coords, `0x0187` add blip for char |
| Handlers | Bloque 100 (`0x00466DE0`), casos `0x36`, `0x37`, `0x3C`, `0x3D`; bloque 300 (`0x0047C100`) para `0x0187` |
| Pool de peds | `CPools::ms_pPedPool` en `0x00B74490` |
| Confianza | **ALTA** en decompilado y firmas; **MEDIA** hasta prueba en juego |
| Binario | GTA SA 1.0 US, SHA-256 `F01A00CE…9343AC` |

> Todos los datos de esta ficha salen de Ghidra 12.2 sobre el ejecutable real y se
> contrastaron con `plugin-sdk` (`CPed.h`, `CPlayerInfo.h`, `CPools.h`) para nombrar
> campos, nunca para inventar direcciones.

---

## 1. Identificadores: `CPool<CPed, CCopPed>`

El juego no usa punteros directos en el SCM: usa **handles de pool**. El pool de
peds es un `CPool<CPed, CCopPed>` (unión de dos tamaños) y su puntero global está
en `0x00B74490`.

Disección de `CPools::GetPed` (`0x0054FF90`, 43 bytes):

```c
int FUN_0054ff90(int handle)
{
  int *pool = *(int **)0x00b74490;      // 8B 15 90 44 B7 00
  if (*(char *)((handle >> 8) + pool[1]) == (char)handle)   // pool[1] = flags/contadores
    return (handle >> 8) * 0x7c4 + pool[0];                 // pool[0] = array de objetos
  return 0;
}
```

Y `CPools::GetPedRef` (`0x0054FF60`) hace el camino inverso:

```c
int FUN_0054ff60(int *ped)
{
  int idx = (ped - *(int **)0x00b74490) / 0x7c4;
  return *(byte *)((*(int **)0x00b74490)[1] + idx) + idx * 0x100;
}
```

Consecuencias verificadas:

- **Stride del pool: `0x7C4`** (tamaño de `CCopPed`; un `CPed` normal ocupa `0x79C`).
- **Handle = `(índice << 8) | contador`**, con el contador en el byte bajo.
  El contador se guarda en el array de flags del pool (`pool+4`, 1 byte por slot).
- Un handle inválido devuelve `0` en `GetPed`; `GetPedRef` siempre devuelve un
  handle válido para un ped vivo del pool.
- El jugador se obtiene de forma canónica con `CWorld::Players[0].m_pPed`
  (`0xB7CD98 + 0x00`), no con el pool. El ped del jugador suele ser el objeto 0
  del pool, pero esa coincidencia no es contrato de la API.

### Funciones limpias para usar desde Rust

| VA | Nombre | Convención | Firma útil |
|---|---|---|---|
| `0x0054FF90` | `CPools::GetPed` | `__cdecl` | `CPed* GetPed(int handle)` |
| `0x0054FF60` | `CPools::GetPedRef` | `__cdecl` | `int GetPedRef(CPed* ped)` |
| `0x00404910` | `CPool<CPed>::GetAt` | `__thiscall` (ECX = pool) | base de las dos anteriores |
| `0x0156CD40` | `CPool<CPed>::GetRef` | `__thiscall` (ECX = pool) | base de las dos anteriores |

---

## 2. `CREATE_CHAR` — opcode `0x009A`

Handler: bloque 100, caso `0x36` (VA del caso dentro de `0x00466DE0`).

### Parámetros (5)

| # | Tipo | Significado |
|---|---|---|
| 1 | `int` | `pedType` (0–21; 6 = policía, 18/19 = tipos especiales) |
| 2 | `int` | `modelId` del ped |
| 3 | `float` | `x` |
| 4 | `float` | `y` |
| 5 | `float` | `z` (`< -100.0` ⇒ calcular suelo) |

Escribe el handle resultante en la variable de salida del SCM.

### Decompilado (recortado)

```c
case 0x36:
  FUN_00464080(5);                                   // leer 5 parámetros
  FUN_00464f50(DAT_00a43c78, &DAT_00a43c7c);         // remapear modelo si pedType == 6 (policía)
  if (DAT_00a43c78 == 6) {                           // 8.40779e-45 == bits de 6.0f
    p = FUN_005e4720(0x7c4);                         // CPool::New(0x7C4)
    p = FUN_005ddc60(p, model);                      // ctor CCopPed
  } else if (DAT_00a43c78 == 18 || DAT_00a43c78 == 19) {
    p = FUN_005e4720(0x79c);                         // CPool::New(0x79C)
    p = FUN_005de340(p, pedType, model);             // ctor CPed B
  } else {
    p = FUN_005e4720(0x79c);
    p = FUN_005ddb70(p, pedType, model);             // ctor CPed C
  }

  task = FUN_0061a5a0(0x20);                         // task alloc (pool de tareas)
  task = FUN_0062f310(999999, 1, 0, 8.0f);           // CTaskSimpleStandStill(counter, ...)
  FUN_00681af0(task, 4, 0);                          // CPed::SetTask(slot 4)
  FUN_005e47e0(2);                                   // CPed::SetCreatedBy(2) -> [ped+0x484]
  *(uint *)(p + 0x46c) &= 0x7fffffff;                // limpiar bit 31

  if (z < -100.0f) z = FUN_00569660(x, y);           // CWorld::FindGroundZ
  pos = { x, y, z + 1.0f };                          // +1.0f SIEMPRE
  FUN_004241c0(&pos);                                // fijar posición
  FUN_00439a80(0, 0, 0);                             // reset de orientación/velocidad
  FUN_00486b00(&pos, p);                             // clear-space (colisión)
  if (script->missionFlag) *(uint *)(p + 0x1c) |= 0x40000;
  FUN_00563220(p);                                   // CWorld::Add
  DAT_00c0ec24++;                                    // contador de peds de misión
  handle = FUN_0156cd40(p);                          // CPool<CPed>::GetRef
  escribir_salida(handle);
```

### Puntos críticos

1. **No pide ni carga el modelo.** El script debe hacer `REQUEST_MODEL` (`0x0247`)
   y esperar `HAS_MODEL_LOADED` (`0x0248`) antes de llamar a `0x009A`.
   Referencia de streaming: `0x4087E0` (`CStreaming::RequestModel`, 2 args) y
   `0x40EA10` (`LoadAllRequestedModels`).
2. **El ctor depende del `pedType`**, no del modelo:
   - `pedType == 6` → `CCopPed`, slot de `0x7C4` bytes, ctor `0x005DDC60`.
   - `pedType == 18 || 19` → ctor `0x005DE340` (slot de `0x79C`).
   - resto → ctor `0x005DDB70` (slot de `0x79C`).
3. **`0x464F50` remapea el modelo** solo para `pedType == 6` (policía): el modelo
   que se pasa al ctor puede no ser el que pidió el script.
4. **Se le asigna una tarea de stand-still** con contador 999999, slot 4, para que
   el ped no se autodestruya por “idle”.
5. **`SetCreatedBy(2)`** (`0x5E47E0`) marca el ped como creado por misión; el
   contador global `0x00C0EC24` se incrementa y se decrementa al borrarlo.
6. **`z` siempre suma `1.0f`** después de `FindGroundZ` o del valor dado.
7. La variable de salida recibe el **handle de pool**, no un puntero.

### Ctor y helpers

| VA | Nombre | Notas |
|---|---|---|
| `0x00464F50` | remapeo de modelo de policía | 4 xrefs; consulta `pedType` y `modelId` |
| `0x005E4720` | thunk de `CPool::New` | 27 xrefs; carga ECX desde `0xB74490` |
| `0x005E45E0` | `CPool::New(size)` | recorre flags/contadores del pool |
| `0x005DDC60` | ctor `CCopPed` | 10 xrefs |
| `0x005DE340` | ctor `CPed` (tipos 18/19) | 6 xrefs |
| `0x005DDB70` | ctor `CPed` (resto) | 11 xrefs |
| `0x005E4880` | `CPed::SetModelIndex` | llamado dentro de los ctors |
| `0x005E47E0` | `CPed::SetCreatedBy` | escribe `[ped+0x484]` |
| `0x0061A5A0` | allocator de tareas | 1772 xrefs |
| `0x0062F310` | ctor `CTaskSimpleStandStill` | args `(999999, 1, 0, 8.0f)` |
| `0x00681AF0` | `CPed::SetTask` | slot 4 |
| `0x00569660` | `CWorld::FindGroundZ` | 64 xrefs |
| `0x00486B00` | clear-space / colisión | 22 xrefs |
| `0x00563220` | `CWorld::Add` | 97 xrefs |

---

## 3. `DELETE_CHAR` — opcode `0x009B`

Handler: bloque 100, caso `0x37`.

```c
case 0x37:
  FUN_00464080(1);
  ped = FUN_00404910(handle);          // CPool<CPed>::GetAt
  FUN_00486240(ped);                   // destruir ped
  if (script->missionFlag)
    FUN_0156e810(handle, 2);           // liberar referencia de misión
```

`0x00486240` (180 bytes, 3 xrefs):

- Saca el ped del vehículo si era el conductor (`CPed+0x460`).
- Limpia tareas (`0x00565510`) y llama al destructor virtual con `flag = 1`.
- Si `[ped+0x484] == 2` (creado por misión), decrementa `0x00C0EC24`.

No se debe usar el handle después de borrar: el contador del slot cambia y
`CPools::GetPed` devolverá `0` (o peor, otro ped si el slot se reutiliza).

---

## 4. `GET_CHAR_COORDS` / `SET_CHAR_COORDS` — `0x00A0` / `0x00A1`

### `0x00A0` (caso `0x3C`)

```c
ped = FUN_00404910(handle);
if (([ped+0x46c] & 0x100) && (vehicle = [ped+0x58c]))
  pos = [vehicle+0x14] ? [vehicle+0x14]+0x30 : vehicle+4;
else
  pos = [ped+0x14] ? [ped+0x14]+0x30 : ped+4;
salida_x = pos[0]; salida_y = pos[1]; salida_z = pos[2];
```

- Si el ped está en un vehículo, devuelve la posición del vehículo.
- Si está “adjunto” a otra entidad (`+0x14`), usa la matriz de esa entidad
  (`+0x30`).
- En caso normal usa la posición del ped (`+0x04/0x08/0x0C`).
- Escribe 3 variables de salida con `0x464370`.

### `0x00A1` (caso `0x3D`)

```c
ped = FUN_00404910(handle);
FUN_00464dc0(ped, x, y, z, 1, 1);     // teletransporte
```

`0x00464DC0` es el helper de colocación/teletransporte (3 xrefs). Los dos
booleanos finales activan la actualización de posición y de colisión.

---

## 5. Blip sobre un actor — `0x0187`

`0x0187` (`ADD_BLIP_FOR_CHAR`) usa `CRadar::SetEntityBlip(2, handle, 1, 3)` y luego
`CRadar::ChangeBlipScale(handle, 3)`. El detalle está en
[`blips.md`](blips.md); aquí solo importa que el blip guarda el **handle de pool**
del ped (`+0x04` de `tRadarTrace`) y desaparece al borrar la entidad si no se
marcó `m_bBlipRemain`.

---

## 6. Secuencia mínima recomendada

```
REQUEST_MODEL(modelo)          # 0x0247  (0x4087E0 con flags 0xC)
# esperar HAS_MODEL_LOADED    # 0x0248  (consulta [0x8E4CD0 + modelo*0x14] == 1)
LOAD_ALL_MODELS_NOW           # 0x0249  (si el script no quiere esperar)
# opcional: LOAD_SCENE(x,y,z) # 0x03D6/0x03D7 fuera del alcance de esta ficha
CREATE_CHAR(tipo, modelo, x, y, z)   # 0x009A -> handle
SET_CHAR_COORDS(handle, x, y, z)     # 0x00A1 (si hace falta reubicar)
ADD_BLIP_FOR_CHAR(handle)            # 0x0187 -> blip handle
...
DELETE_CHAR(handle)                  # 0x009B
```

---

## 7. Límites y riesgos

- **No hay validación de `pedType`** en el handler: valores fuera de los casos
  especiales caen en el ctor genérico. Un `pedType` fuera de rango puede producir
  un ped sin comportamiento o crashear al primer tick.
- **El modelo debe estar cargado**: el ctor llama a `SetModelIndex` sin comprobar
  streaming.
- **El pool puede estar lleno**: `CPool::New` devuelve `0`; el handler deja el
  handle en 0 y sigue (no crashea, pero el script recibe un handle inválido).
- **Borrar dos veces** el mismo handle: la segunda llamada obtiene `0` de
  `CPools::GetPed`, pero el handler no comprueba `0` antes de llamar a
  `0x00486240`; el destructor sí comprueba `this != 0` (primera instrucción),
  por lo que no debería crashear. Aun así, no se debe hacer.
- **`0x00C0EC24`** es un contador global de peds de misión; crear/borrar peds con
  `SetCreatedBy(2)` lo mantiene. No tocarlo a mano.

---

## 8. Firmas verificadas

| VA | Símbolo | Prólogo (16 bytes) | Xrefs |
|---|---|---|---|
| `0x0054FF90` | `CPools::GetPed` | `8B 4C 24 04 8B 15 90 44 B7 00 53 56 8B 72 04 8B` | 0 (llamado por script vía thunks) |
| `0x0054FF60` | `CPools::GetPedRef` | `8B 4C 24 04 56 8B 35 90 44 B7 00 2B 0E B8 A7 7C` | 1 |
| `0x005E45E0` | `CPool::New` | `8B 51 08 56 32 C0 57 8B 79 0C 47 8B F7 3B F2 89` | 0 (thiscall) |
| `0x005E4720` | thunk `CPool::New` | `8B 0D 90 44 B7 00 E9 B5 FE FF FF 90 90 90 90 90` | 27 |
| `0x00464F50` | remapeo modelo policía | `8B 44 24 08 8B 08 81 C1 E8 FE FF FF 83 F9 07 77` | 4 |
| `0x005DDC60` | ctor `CCopPed` | `6A FF 68 88 D7 83 00 64 A1 00 00 00 00 50 64 89` | 10 |
| `0x005DE340` | ctor `CPed` B | `6A FF 68 C8 D7 83 00 64 A1 00 00 00 00 50 64 89` | 6 |
| `0x005DDB70` | ctor `CPed` C | `6A FF 68 68 D7 83 00 64 A1 00 00 00 00 50 64 89` | 11 |
| `0x005E4880` | `CPed::SetModelIndex` | `56 8B F1 81 4E 1C 80 00 00 00 8B 44 24 08 57 50` | 7 |
| `0x005E47E0` | `CPed::SetCreatedBy` | `56 8B F1 8B 86 98 05 00 00 85 C0 8A 4C 24 08 88` | 23 |
| `0x0061A5A0` | allocator de tareas | `8B 0D A8 44 B7 00 E9 55 FF FF FF 90 90 90 90 90` | 1772 |
| `0x0062F310` | ctor `CTaskSimpleStandStill` | `56 8B F1 E8 78 B0 FE FF 8B 44 24 08 8A 4C 24 0C` | 129 |
| `0x00681AF0` | `CPed::SetTask` | `53 8B 5C 24 08 85 DB 56 57 8B 7C 24 14 8B F1 75` | 77 |
| `0x00569660` | `CWorld::FindGroundZ` | `83 EC 38 8B 44 24 3C 8B 4C 24 40 6A 00 6A 00 6A` | 64 |
| `0x00486B00` | clear-space | `A0 78 57 A9 00 83 EC 50 A8 01 75 07 83 0D 78 57` | 22 |
| `0x00563220` | `CWorld::Add` | `56 8B 74 24 08 8B 46 18 85 C0 74 1D 8B 40 04 8B` | 97 |
| `0x00486240` | destructor de ped | `56 8B 74 24 08 85 F6 0F 84 A5 00 00 00 8A 8E 84` | 3 |
| `0x00464DC0` | teletransporte | `83 EC 0C 56 57 8B 7C 24 18 90 E9 E2 34 FA FF F6` | 3 |

---

## 9. Qué falta para exponerlo en RustyScript

1. Un wrapper `__cdecl` de `CPools::GetPed`/`GetPedRef` ya es viable (no requiere
   reconstruir el pool).
2. `CREATE_CHAR` necesita replicar la secuencia completa: asignar slot, llamar al
   ctor correcto, crear tarea, `SetTask`, `SetCreatedBy(2)`, fijar posición,
   `CWorld::Add`. **No** conviene llamar al handler del SCM directamente porque
   depende del estado del hilo de script (`DAT_00A43C78…`).
3. Antes de escribir la nativa hay que probar en juego: crear un ped con modelo
   precargado, leer sus coordenadas, ponerle un blip y borrarlo.
