# CCheat Vehicle Spawn — stubs y función core

## Stubs de spawn (funciones hoja de 12 bytes)

Patrón: `68 <model_id:u32> E8 <rel32> 59 C3`  
= PUSH model_id ; CALL VEHICLE_CHEAT_CORE ; POP ECX ; RET

Encontrados 16 stubs mediante búsqueda de patrón en .text:

| VA           | Model ID | Vehículo              |
|--------------|----------|-----------------------|
| `0x0043A4A0` | 432      | Rhino (tanque)        |
| `0x0043A4B0` | 504      | Bloodring Racer       |
| `0x0043A4C0` | 505      | Bloodring Banger      |
| `0x0043A4D0` | 502      | Hotring Racer A       |
| `0x0043A4E0` | 503      | Hotring Racer B       |
| `0x0043A4F0` | 442      | Monster Truck         |
| `0x0043A500` | 409      | Stretch (limusina)    |
| `0x0043A510` | 408      | Bennie (furgoneta)    |
| `0x0043A520` | 457      | Caddy (golf)          |
| `0x0043A530` | 520      | Hydra (jet)           |
| `0x0043A540` | 539      | Dozer                 |
| `0x0043A550` | 425      | Hunter (helicóptero)  |
| `0x0043A560` | 471      | Quad                  |
| `0x0043A660` | 486      | Bike (moto)           |
| `0x0043A670` | 513      | Camper                |
| `0x0043A680` | 556      | Monster B             |

**Todos los stubs llaman a `0x0043A0B0`** (VEHICLE_CHEAT_CORE).

## Tabla de hashes de cheats

Los stubs están referenciados en la tabla de hashes de cheats en  
`.data` @ `0x008A5B9C`:
```
0x008A5B9C: 0x0043A4A0  0x0043A4B0   (Rhino, Bloodring Racer)
0x008A5BA4: 0x0043A4C0  0x0043A4D0   (Bloodring Banger, Hotring A)
...
```
Esto confirma que estos stubs son los handlers directos de los códigos de truco.

## VEHICLE_CHEAT_CORE @ 0x43A0B0

| Campo               | Valor                                                     |
|---------------------|-----------------------------------------------------------|
| Dirección (VA)      | `0x0043A0B0`                                              |
| Nivel de confianza  | **MEDIA** — pendiente prueba en juego                     |
| Convención          | `__cdecl`, arg: (model_id: i32)                           |

La función recibe el model_id en la pila (el PUSH del stub lo coloca), lo que  
es consistente con `__cdecl`. Llama internamente a las funciones de creación  
de vehículo y posicionamiento junto al jugador.

**No activar en game.rs hasta que el usuario confirme en juego.**

## Nota: 0x439390 NO es VehicleCheat

La dirección `0x439390` que el mod previo registraba como VehicleCheat  
es en realidad el interior de la función `0x439366` (file offset `0x38766`),  
que llena un array en la pila con valores `0x66..0x6E` (códigos de ropa).  
No tiene callers directos encontrados, probablemente es un helper inline.  
Firma: `83 EC 20 84 D2 0F 94 C0 ...`
