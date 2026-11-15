# RustyScript

**Lenguaje de scripting embebido en Rust para modificar GTA San Andreas 1.0 US en tiempo de ejecución.**

RustyScript es un plugin `.asi` (`rustymod.dll` → `rustymod.asi`) escrito en Rust `no_std`,
sin dependencias externas, que se engancha al bucle principal del juego y ejecuta scripts
`.rscript` escritos en un lenguaje propio, sencillo y seguro por defecto.

La idea es que cualquiera pueda modificar el juego con archivos de texto como este:

```rscript
# salud.rscript — restaura salud y armadura con F6/F7

on key F6 {
    player.health = player.max_health
    show_help("Salud restaurada")
}

on key F7 {
    player.armor = 100.0
    show_help("Armadura restaurada")
}
```

...sin tocar punteros, direcciones de memoria ni recompilar el mod.

---

## Qué incluye

| Componente | Descripción |
|---|---|
| **Hook del frame** | Parchea el único `CALL` a `CGame::Process` (`0x53E981` → `0x53BEE0`) y encadena cualquier hook previo (CLEO u otro ASI) leyendo el destino real en memoria. |
| **Lenguaje `.rscript`** | Lexer, parser, compilador a bytecode y VM de pila propios, todo en Rust `no_std` + `alloc`. |
| **Eventos** | `on start`, `on frame`, `on key <tecla>`, `on held <tecla>`. |
| **Nativas del juego** | Salud, armadura, posición, HUD, hora, clima y spawn de vehículos por truco. |
| **Aislamiento de fallos** | Presupuesto de 2 ms por frame, pausa/reanudación y desactivación del script tras 3 errores en 5 s. Sin cerrar el juego. |
| **Log** | `RustyMod.log` junto al `.asi`, con firmas, bytes del parche y trazas del intérprete. |

---

## Estado del proyecto: **Fase 3 — intérprete de scripts integrado (validación en curso)**

Fases según el avance real del repositorio:

| Fase | Contenido | Estado |
|---|---|---|
| **Fase 0** | Verificación del ejecutable y análisis binario (`tools/`, `docs/evidence/`) | ✅ Completada |
| **Fase 1** | Hook estable en `CGame::Process` con encadenado de otros ASI | ✅ Completada y probada en juego |
| **Fase 2** | Acceso al jugador, HUD, teclas y log | ✅ Completada — salud y armadura confirmadas en juego (F6/F7) |
| **Fase 3** | Lenguaje RustyScript: lexer, parser, compilador, VM, loader y nativas | 🟡 **Implementada y ejecutándose en juego**; quedan pruebas visuales y nativas de confianza MEDIA |
| **Fase 4** | Configuración por archivo `.toml`, distribución y pulido | ⬜ No iniciada |

Evidencia de la fase actual (log real de una partida, `RustyMod.log`):

```
RustyMod: hook instalado en 0x0053E981 -> 0x5D9B1000
RustyMod: primer frame - cargando scripts
ScriptEngine: loaded 'scripts\mundo.rscript' OK
ScriptEngine: loaded 'scripts\secuencia.rscript' OK
ScriptEngine: loaded 'scripts\vehiculos.rscript' OK
VM: CallNative(SetTime, argc=2)  -> RustyScript: set_time(0, 0)
VM: CallNative(SpawnRhino, argc=0) -> RustyScript: spawn @ 0x0043A4A0
```

### Pendiente para cerrar la Fase 3

- Confirmar visualmente que `show_help` / `show_message` muestran el texto en pantalla.
- Confirmar que los vehículos spawneados aparecen realmente junto al jugador.
- Validar la reanudación de `wait()` con el presupuesto de frame.
- Probar las nativas de confianza MEDIA antes de activarlas (`player.money`, `player.wanted_level`, `spawn_vehicle`).
- Añadir pruebas automatizadas del lexer/parser/compilador/VM (hoy no hay `cargo test`).
- Corregir la asignación de slots de variables locales en bloques anidados.
- Implementar el límite de 16 scripts y los diagnósticos `W02` / `E12`.

---

## Arquitectura

```
src/
├── lib.rs            DllMain + panic handler (no_std)
├── hook.rs           Hook del CALL a CGame::Process, tick por frame, ScriptEngine global
├── game.rs           Direcciones verificadas del binario (con ficha de evidencia)
├── player.rs         Lectura/escritura segura de CPed y CPlayerInfo
├── hud.rs            SetHelpMessage / AddMessageJumpQ
├── controls.rs       GetAsyncKeyState, teclas virtuales
├── config.rs         Teclas y presupuesto por frame (constantes)
├── allocator.rs      GlobalAlloc sobre HeapAlloc de Windows
├── log.rs            Log síncrono a RustyMod.log
└── script/
    ├── lexer.rs      Tokenizador
    ├── parser.rs     Parser recursivo descendente → AST
    ├── compiler.rs   AST → bytecode (Opcode) + chequeos semánticos
    ├── vm.rs         Máquina de pila, frames, wait(), presupuesto de tiempo
    ├── natives.rs    Implementación de cada función nativa
    └── loader.rs     FindFirstFileA / CreateFileA / ReadFile
```

`docs/rrustyscript-spec.md` es la especificación del lenguaje v0.1; `docs/natives.md` y
`docs/natives.toml` el catálogo de nativas; `docs/evidence/` las fichas de cada dirección
(firma de bytes, xrefs y nivel de confianza).

---

## Lenguaje (resumen)

```rscript
var avisado = false

fn curar(umbral: float) {
    if player.health < umbral {
        player.health = player.max_health
    }
}

on start {
    show_help("RustyScript cargado")
}

on frame {
    if not avisado and player.health < 25.0 {
        curar(50.0)
        avisado = true
    }
}

on key F2 {
    spawn_rhino()
    wait(1000)          # cede el control y reanuda en el siguiente frame
    spawn_hydra()
}
```

- Tipos: `int`, `float`, `bool`, `string` (más `nil` interno).
- `var` (global del script), `let` (local), `fn` (máx. 32, sin recursión).
- Operadores: `+ - * / %`, comparación, `and or not`, `= += -= *= /=`.
- Control: `if / else if / else`, `while`, `break`, `continue`, `return`.
- Eventos: `on start`, `on frame`, `on key F1..F12 / NUM0..NUM9`, `on held`.
- `wait(ms)` es una extensión implementada del lenguaje (aún no reflejada en la spec v0.1).

### Nativas activas

| Grupo | Funciones |
|---|---|
| Jugador | `player.health` (r/w), `player.max_health` (r), `player.armor` (r/w), `player.x/y/z` (r) |
| Pantalla | `show_help(texto)`, `show_message(texto, ms)` |
| Mundo | `set_time(hora, minuto)`, `get_hour()`, `get_minute()`, `set_weather(id)` |
| Vehículos | `spawn_rhino()`, `spawn_hydra()`, `spawn_hunter()`, `spawn_monster()`, `spawn_hotring()`, `spawn_stretch()`, `spawn_caddy()` |
| Control | `wait(ms)` |

Desactivadas o ausentes en esta versión: `player.money`, `player.wanted_level`,
`spawn_vehicle(id)`, `give_weapon(id, ammo)`. Se activarán solo tras confirmarlas en juego.

---

## Compilar

Requisitos: Rust estable + target `i686-pc-windows-msvc` (GTA SA es 32 bits).

```powershell
rustup target add i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

Resultado: `target\i686-pc-windows-msvc\release\rustymod.dll`.

Instalación:

```powershell
Copy-Item "target\i686-pc-windows-msvc\release\rustymod.dll" `
          "<ruta del juego>\rustymod.asi" -Force
```

Coloca los `.rscript` en `<ruta del juego>\scripts\`. Al arrancar se cargan todos y se
recargan con **F9**. Se escribe `RustyMod.log` junto al `.asi`.

En `scripts/` hay ejemplos listos para copiar: `salud.rscript`, `mundo.rscript`,
`vehiculos.rscript` y `secuencia.rscript` (demostración de `wait()`).

Teclas del núcleo (independientes de los scripts): **F6** salud, **F7** armadura,
**F9** recargar scripts.

---

## Direcciones verificadas

Solo entran en `src/game.rs` con ficha en `docs/evidence/`. Objetivo: **GTA SA 1.0 US**,
SHA-256 `F01A00CE950FA40CA1ED59DF0E789848C6EDCF6405456274965885D0929343AC`,
ImageBase `0x00400000`, sin ASLR.

| Elemento | Dirección / offset | Confianza |
|---|---|---|
| `CGame::Process` | `0x53BEE0` | ALTA |
| Punto de hook (`CALL`) | `0x53E981` | ALTA |
| `CHud::SetHelpMessage` | `0x588BE0` | ALTA |
| `CMessages::AddMessageJumpQ` | `0x69F1E0` | ALTA |
| `CWorld::Players` | `0xB74490` (doble deref) | ALTA |
| `CPed` health / max / armor | `+0x540` / `+0x544` / `+0x548` | ALTA |
| `CPed` posición x/y/z | `+0x44` / `+0x48` / `+0x4C` | ALTA |
| `CClock` horas / minutos | `0xB70153` / `0xB70152` | ALTA |
| `CWeather` old / new | `0xC81320` / `0xC81324` | ALTA |
| Stubs de spawn por truco | `0x43A4A0` … `0x43A550` | ALTA |
| `CPlayerInfo::m_nMoney` | `+0xB8` | MEDIA (desactivado) |

---

## Herramientas de análisis

`tools/` contiene los scripts de Python usados para verificar el binario (secciones PE,
búsqueda de firmas, callers, xrefs y offsets) y `tools/verify_all.py` como verificación
exhaustiva del catálogo. `docs/REPRODUCIBLE.md` documenta los comandos exactos.

El ejecutable del juego **no** se distribuye en este repositorio: hay que usar una copia
propia de GTA San Andreas 1.0 US. `work/` está excluido por `.gitignore`.

---

## Estado legal

Proyecto no oficial, sin relación con Rockstar Games ni Take-Two. No incluye código,
assets ni binarios del juego. Requiere una copia legítima de GTA San Andreas 1.0 US.
Úsalo bajo tu responsabilidad y solo en modo un jugador.
