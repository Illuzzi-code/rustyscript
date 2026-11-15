# show_message: por qué no se dibujaba el texto (lifetime del puntero)

| Campo              | Valor                                                        |
|--------------------|--------------------------------------------------------------|
| Síntoma            | `show_help("...")` muestra texto; `show_message("...", ms)` no muestra nada |
| Función implicada  | `CMessages::AddMessageJumpQ @ 0x0069F1E0`                     |
| Estado global      | `DAT_00C1A7F0` (texto), `DAT_00C1A7F8` (duración), `DAT_00C1A7FC` (timestamp) |
| Causa raíz         | `AddMessageJumpQ` **no copia** el texto: guarda el puntero del llamante |
| Nivel de confianza | **ALTA** (decompilado con Ghidra 12.2 sobre el exe real)       |
| Binario            | GTA SA 1.0 US, SHA-256 `F01A00CE…9343AC`                      |

## 1. Decompilado de `FUN_0069F1E0` (AddMessageJumpQ)

```c
void FUN_0069f1e0(undefined4 param_1, undefined4 param_2, undefined2 param_3,
                  char param_4)
{
  char  local_320;            // buffer LOCAL de 800 bytes en la pila
  char  local_31f[799];

  FUN_0069db70(&local_320, param_1, 400);  // copia ACOTADA a un buffer local
  FUN_0069e160(&local_320);                // procesa códigos de color ~...~
  pcVar2 = &local_320;                     // recorre la COPIA local
  while (local_320 != '\0') { ... }

  DAT_00c1a7fc = DAT_00b7cb84;             // timestamp
  DAT_00c1a7f8 = param_2;                  // duración en ms (32 bits)
  DAT_00c1a7f0 = param_1;                  // ← guarda el puntero ORIGINAL
  DAT_00c1a7f4._0_2_ = param_3;            // flags
  ...
  DAT_00c1a81c = param_4;
  if (param_4 != '\0') {
    FUN_0069dd50(param_1, -1, -1, -1, -1, -1, -1, 0); // briefs: guarda el MISMO puntero
  }
  return;
}
```

Puntos clave:

* `FUN_0069DB70(dst, src, ushort max)` es una copia acotada: copia como mucho
  `max - 1` bytes y añade el NUL. Aquí `max = 400`, pero el destino es
  `&local_320`, **una variable local** que desaparece al retornar.
* El texto que queda en la cola de mensajes es `param_1`, es decir, **el puntero
  que pasó el llamante**, no la copia local.
* `param_2` (duración) se guarda como dword (`DAT_00C1A7F8`), no como `u16`.

## 2. El render lee ese puntero en frames posteriores

`DAT_00C1A7F0` tiene **28 referencias** en `.text`:

```
0x0046B572
0x0069E3DB 0x0069E414 0x0069E4BD 0x0069E560 0x0069E86B 0x0069E8F1 0x0069E935
0x0069E9D9 0x0069EA5B 0x0069EA76 0x0069EACC 0x0069EAF0 0x0069EB0D 0x0069EB1B
0x0069EB80 0x0069EBB0 0x0069EBC8 0x0069EEF0 0x0069EF17 0x0069EF40 0x0069EF58
0x0069EF61 0x0069F070 0x0069F0FC 0x0069F181 0x0069F1C9 0x0069F24D
```

La cola de mensajes es un array de **8 entradas** de 48 bytes
(`FUN_0069E360`: `if (sVar4 < 8)`), con esta forma por entrada:

| Offset          | Campo                          |
|-----------------|--------------------------------|
| `+0x00`         | puntero al texto               |
| `+0x04`         | flags (`u16`)                  |
| `+0x08`         | duración en ms (`u32`)         |
| `+0x0C`         | timestamp de entrada           |
| `+0x10`…`+0x24` | parámetros de formato          |
| `+0x30`         | añadir a briefs                |

Además, los *briefs* (`FUN_0069DD50`) guardan también el puntero
(`(&DAT_00c1a570)[slot*8] = param_1`) en una lista de **20** entradas
(`if (0x13 < uVar3) break`).

Conclusión: el buffer debe seguir siendo válido no solo durante la llamada, sino
durante todos los frames en que el mensaje esté visible.

## 3. Así llama el propio juego

`FUN_005B1720` (procesador de mensajes encolados por script):

```c
uVar3 = FUN_006a0050(&DAT_00bc2fc0 + DAT_00bc3ff0 * 8); // CTheText::Get(key)
FUN_0069f1e0(uVar3, &DAT_00bc2dc0[...], 1, 1);          // (texto, ms, 1, 1)
```

`CTheText::Get @ 0x6A0050` devuelve un `char*` ya resuelto, y ese puntero se
pasa tal cual a `AddMessageJumpQ`. Por tanto **no hace falta que el texto sea una
clave GXT**: el juego acepta texto ya resuelto, y de hecho lo usa así.

## 4. Causa del bug en v0.1

```rust
let buf = to_cstring(text, 127);      // Vec<u8> temporal
fn_ptr(buf.as_ptr(), time_ms, 1, 0);  // el juego guarda buf.as_ptr()
// buf se libera al salir de la nativa → puntero colgante
// frames posteriores leen memoria liberada → no se dibuja texto
```

`show_help` no sufre esto porque `CHud::SetHelpMessage @ 0x588BE0` copia el texto
a sus propios buffers (`FUN_0069DB70` + `FUN_0069E160` sobre estructuras del
HUD), y no conserva el puntero del llamante.

## 5. Corrección aplicada

`src/script/natives.rs`:

* Ring estático `MSG_ARENA[32][400]` con índice `MSG_NEXT`.
* El texto se copia ahí (codificado en CP1252) y se pasa el puntero estable.
* 400 bytes = límite real de `FUN_0069DB70`; 32 slots cubren las 8 entradas de
  la cola + las 20 de briefs sin reutilizar texto todavía alcanzable.
* Llamada fiel al juego: `AddMessageJumpQ(ptr, time_ms: u32, 1, 1)`.

`src/hud.rs::show_message` queda como envoltorio de bajo nivel y documenta que el
puntero debe seguir vivo; el camino con texto dinámico pasa por la nativa.

## 6. Nota de codificación

GTA SA dibuja con Windows-1252, no UTF-8. La nativa convierte `á é í ó ú ñ ¿ ¡`
a su byte CP1252 (rango Latin-1, identidad) para que no salgan dos glifos
basura. Caracteres fuera de CP1252 se sustituyen por `?`.

## 7. Verificación estática

* Firma en `0x0069F1E0`: `81 EC 20 03 00 00 56 8B B4 24 28 03 00 00 68 90`
  (file offset `0x29E5E0`).
* `add esp, 10h` en los llamantes (`0x004681BE`, `0x005B187E`) → `__cdecl`,
  4 argumentos de 4 bytes.
* Prueba en juego: **pendiente** (requiere ejecutar GTA SA y mirar la pantalla).
