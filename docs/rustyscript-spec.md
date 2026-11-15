# RustyScript — Especificación del Lenguaje v0.1

RustyScript es un lenguaje de scripting para modificar GTA San Andreas en tiempo
de ejecución. Se ejecuta dentro del hilo del juego, una vez por frame.

---

## Principios de diseño

- **Legible sin conocer Rust** — sintaxis inspirada en Python/Lua, sin llaves en la mayoría de los contextos
- **Seguro por defecto** — el script nunca toca punteros ni direcciones; solo valores
- **Límite de tiempo por frame** — si un script tarda demasiado, se pausa y registra el error
- **Fallo aislado** — un error en un script lo desactiva sin cerrar el juego ni afectar otros scripts
- **Fácil de aprender** — el ejemplo de bienvenida funciona en 3 líneas

---

## 1. Estructura de un archivo `.rscript`

```
# comentario de línea

on start {
    # se ejecuta una sola vez cuando el script se carga
}

on frame {
    # se ejecuta cada frame (unas 30-60 veces por segundo)
}

on key F6 {
    # se ejecuta cuando se pulsa F6 (solo el primer frame)
}
```

Los bloques `on start`, `on frame` y `on key` son los únicos puntos de entrada.
Todo el código ejecutable debe estar dentro de uno de ellos.
El código fuera de estos bloques solo puede ser declaraciones de variables globales o funciones.

---

## 2. Tipos

| Tipo     | Descripción                        | Ejemplo literal  |
|----------|------------------------------------|------------------|
| `int`    | Entero con signo 32 bits           | `42`, `-7`       |
| `float`  | Flotante 32 bits                   | `3.14`, `-0.5`   |
| `bool`   | Booleano                           | `true`, `false`  |
| `string` | Cadena de texto UTF-8 inmutable    | `"hola mundo"`   |

No hay punteros, referencias ni tipos compuestos en la v0.1.

### Conversiones implícitas
- `int` → `float` en expresiones mixtas
- `float` → `int` al asignar a variable `int` (trunca, no redondea)

---

## 3. Variables

```
# variable global del script (vive entre frames)
var salud_anterior = 0.0

on frame {
    # variable local (vive solo en este bloque)
    let hp = player.health
    if hp < 30.0 {
        show_help("¡Salud baja!")
    }
}
```

- `var` declara una variable de script (persiste entre frames y eventos)
- `let` declara una variable local al bloque actual
- El tipo se infiere del valor asignado; no cambia después
- No hay variables globales entre scripts distintos

---

## 4. Operadores

### Aritméticos
```
+   -   *   /   %
```

### Comparación
```
==   !=   <   >   <=   >=
```

### Lógicos
```
and   or   not
```

### Asignación
```
=    +=   -=   *=   /=
```

---

## 5. Expresiones y precedencia

De mayor a menor:
1. `not`, `-` (unario)
2. `*`, `/`, `%`
3. `+`, `-`
4. `<`, `>`, `<=`, `>=`, `==`, `!=`
5. `and`
6. `or`

Usar paréntesis para cambiar el orden: `(a + b) * c`

---

## 6. Control de flujo

### if / else if / else

```
if player.health < 50.0 {
    show_help("Salud baja")
} else if player.health < 25.0 {
    show_help("¡PELIGRO!")
} else {
    show_help("Salud OK")
}
```

### while

```
var i = 0
while i < 10 {
    i += 1
}
```

### break y continue

```
while true {
    if player.health >= 100.0 {
        break
    }
}
```

---

## 7. Funciones definidas en el script

```
fn curar_si_bajo(umbral: float) {
    if player.health < umbral {
        player.health = player.max_health
        show_help("Curado automáticamente")
    }
}

on key F6 {
    curar_si_bajo(50.0)
}
```

- Las funciones pueden devolver un valor con `return`; un `return` sin expresión devuelve `nil`
- No hay recursión (detectada en el analizador semántico, genera error en carga)
- Máximo 32 funciones por script

---

## 8. Eventos

| Evento        | Cuándo se ejecuta                                     |
|---------------|-------------------------------------------------------|
| `on start`    | Una vez al cargar el script o al recargar con F9      |
| `on frame`    | Cada frame del juego (dentro del límite de tiempo)    |
| `on key <K>`  | El primer frame en que se pulsa la tecla K            |
| `on held <K>` | Cada frame en que la tecla K esté mantenida           |

### Teclas válidas
`F1` `F2` `F3` `F4` `F5` `F6` `F7` `F8` `F9` `F10` `F11` `F12`
`NUM0`–`NUM9`

---

## 9. Funciones nativas disponibles

Solo las de confianza ALTA están activas. Las demás generan error en carga.

### Jugador

```
player.health        # float, lectura y escritura (clampado a max_health)
player.max_health    # float, solo lectura
player.armor         # float, lectura y escritura (clampado a 100.0)
player.x             # float, solo lectura
player.y             # float, solo lectura
player.z             # float, solo lectura
```

### Pantalla

```
show_help(texto: string)
# Muestra el texto en la caja amarilla de ayuda.
# El texto puede ser cualquier cadena UTF-8 (máx 127 chars).

show_message(texto: string, tiempo_ms: int)
# Muestra un mensaje en la parte inferior izquierda durante tiempo_ms ms.
```

### Mundo

```
set_time(hora: int, minuto: int)
# Cambia la hora del juego. hora: 0-23, minuto: 0-59.

get_hour() -> int
get_minute() -> int
# Devuelven la hora y minuto actuales.

set_weather(id: int)
# Cambia el clima. IDs: 0=extra soleado, 1=soleado, 2=nublado,
# 3=lluvioso, 4=tormenta, 9=niebla, 10=muy nublado, 11=desierto.
```

### Vehículos

```
spawn_rhino()       # Spawna un Rhino (tanque) junto al jugador
spawn_hydra()       # Spawna un Hydra (jet)
spawn_hunter()      # Spawna un Hunter (helicóptero de combate)
spawn_monster()     # Spawna un Monster Truck
spawn_hotring()     # Spawna un Hotring Racer
```

### Funciones desactivadas (MEDIA — pendiente prueba)

Estas aparecen en la especificación pero generan `error[E01]` si se usan:
```
player.money        # i32
player.wanted_level # int 0-6
spawn_vehicle(model_id: int)
```

---

## 10. Modelo de seguridad

- El script **nunca** recibe punteros ni direcciones de memoria
- Escrituras a campos del jugador se clampan a rangos seguros antes de ejecutarse
- Si un script tarda más de **2 ms por frame**, se interrumpe y se registra `warn[W01]`
- Si un script lanza un error en tiempo de ejecución, se desactiva con `error[E10]` en el log — el juego no se cierra
- Si un script causa más de 3 errores en 5 segundos, se desactiva permanentemente hasta recargar
- `show_message` requiere exactamente `(texto, tiempo_ms)` y acepta `tiempo_ms` entre 0 y 65535

---

## 11. Errores

Todos los errores incluyen el nombre del script y el número de línea.

### Errores en carga (`error[E0x]`)

| Código | Mensaje                                              |
|--------|------------------------------------------------------|
| E01    | función nativa no disponible en esta versión         |
| E02    | variable no declarada: `<nombre>`                    |
| E03    | tipos incompatibles: `<tipo_a>` y `<tipo_b>`         |
| E04    | demasiadas funciones (máximo 32)                     |
| E05    | recursión detectada en función `<nombre>`            |
| E06    | bloque `on key` con tecla desconocida: `<K>`         |
| E07    | archivo vacío o sin bloques de evento                |

### Errores en ejecución (`error[E1x]`)

| Código | Mensaje                                              |
|--------|------------------------------------------------------|
| E10    | error interno en script (script desactivado)         |
| E11    | división por cero en línea `<N>`                     |
| E12    | valor fuera de rango en `<función>`                  |
| E13    | jugador no disponible (en menú o muerto)             |

### Avisos (`warn[W0x]`)

| Código | Mensaje                                              |
|--------|------------------------------------------------------|
| W01    | script superó el límite de tiempo (2ms), pausado     |
| W02    | variable `<nombre>` declarada pero nunca usada       |

---

## 12. Ejemplos completos

### Ejemplo 1 — Restaurar salud y armadura

```
# salud.rscript
# Restaura salud y armadura con F6/F7.

on key F6 {
    player.health = player.max_health
    show_help("Salud restaurada")
}

on key F7 {
    player.armor = 100.0
    show_help("Armadura restaurada")
}
```

### Ejemplo 2 — Autosalud cuando está baja

```
# autosalud.rscript
var avisado = false

on frame {
    if player.health < 25.0 {
        if not avisado {
            show_help("¡PELIGRO! Curando...")
            avisado = true
        }
        player.health = player.max_health
    } else {
        avisado = false
    }
}
```

### Ejemplo 3 — Noche perpetua

```
# noche.rscript
on key F5 {
    set_time(0, 0)
    set_weather(2)
    show_help("Son las 00:00")
}
```

### Ejemplo 4 — Mostrar posición

```
# posicion.rscript
on key F8 {
    let x = player.x
    let y = player.y
    show_help("Posicion registrada en log")
    # Las variables no se pueden interpolar en strings en v0.1,
    # pero el log de Rust sí las registra (ver on_frame en el .asi).
}
```

### Ejemplo 5 — Spawn de vehículos

```
# vehiculos.rscript
on key F3 {
    spawn_rhino()
    show_help("Rhino invocado")
}

on key F4 {
    spawn_hydra()
    show_help("Hydra invocada")
}
```

---

## 13. Formato de archivo

- Extensión: `.rscript`
- Codificación: UTF-8
- Ubicación: carpeta `scripts/` junto al archivo `rustymod.asi`
- Todos los `.rscript` en esa carpeta se cargan al arrancar el juego
- Recarga con **F9** (reconfigurable en `config.rs`)
- Máximo **16 scripts** simultáneos

---

## 14. Gramática (referencia)

```
program     = (decl | event_block)*
decl        = "var" IDENT "=" expr NEWLINE
            | "fn" IDENT "(" param_list ")" block
event_block = "on" event_kind block
event_kind  = "start"
            | "frame"
            | "key" KEY_NAME
            | "held" KEY_NAME
block       = "{" stmt* "}"
stmt        = let_stmt | assign_stmt | if_stmt | while_stmt
            | break_stmt | continue_stmt | call_stmt | NEWLINE
let_stmt    = "let" IDENT "=" expr NEWLINE
assign_stmt = IDENT assign_op expr NEWLINE
            | field assign_op expr NEWLINE
assign_op   = "=" | "+=" | "-=" | "*=" | "/="
if_stmt     = "if" expr block ("else if" expr block)* ("else" block)?
while_stmt  = "while" expr block
break_stmt  = "break" NEWLINE
continue_stmt = "continue" NEWLINE
call_stmt   = IDENT "(" arg_list ")" NEWLINE
            | field "=" expr NEWLINE
field       = "player" "." FIELD_NAME
expr        = or_expr
or_expr     = and_expr ("or" and_expr)*
and_expr    = cmp_expr ("and" cmp_expr)*
cmp_expr    = add_expr (CMP_OP add_expr)?
add_expr    = mul_expr (("+"|"-") mul_expr)*
mul_expr    = unary (("*"|"/"|"%") unary)*
unary       = ("-"|"not") unary | primary
primary     = INT_LIT | FLOAT_LIT | BOOL_LIT | STRING_LIT
            | IDENT | field | IDENT "(" arg_list ")" | "(" expr ")"
param_list  = (IDENT ":" TYPE ("," IDENT ":" TYPE)*)?
arg_list    = (expr ("," expr)*)?
TYPE        = "int" | "float" | "bool" | "string"
KEY_NAME    = "F1".."F12" | "NUM0".."NUM9"
```

---

## 15. Hoja de referencia rápida

```
# variables
var x = 0           # global del script
let y = 1.5         # local al bloque

# control
if cond { }
while cond { }
break  /  continue

# funciones propias
fn nombre(a: int, b: float) { ... }

# eventos
on start { }
on frame { }
on key F5 { }
on held F5 { }

# jugador
player.health       player.max_health
player.armor        player.x  .y  .z

# pantalla
show_help("texto")
show_message("texto", 3000)

# mundo
set_time(hora, minuto)
get_hour()   get_minute()
set_weather(id)

# vehículos
spawn_rhino()   spawn_hydra()
spawn_hunter()  spawn_monster()  spawn_hotring()
```
