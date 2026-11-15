# CGame::Process

| Campo               | Valor                                                        |
|---------------------|--------------------------------------------------------------|
| Dirección (VA)      | `0x0053BEE0`                                                 |
| File offset         | `0x13B2E0`                                                   |
| Firma (16 bytes)    | `83 EC 0C 53 56 57 E8 E5 5E 00 00 B9 78 29 B7 00`           |
| Convención          | `__cdecl`, sin argumentos, sin valor de retorno              |
| Nivel de confianza  | **ALTA**                                                     |
| Prueba en juego     | Pendiente (FASE 0 / FASE 1)                                  |

## Evidencia

### Firma de bytes
Lectura directa del binario en file offset 0x13B2E0:
```
83 EC 0C    sub esp, 0Ch
53          push ebx
56          push esi
57          push edi
E8 E5 5E    call +0x5EE5     ; llamada interna al arranque del frame
```
Prólogo coherente con una función grande que administra el estado global del juego.

### Caller único
Búsqueda exhaustiva de `E8 xx xx xx xx` con target `0x53BEE0` en todas las secciones ejecutables: **1 único resultado** en `0x53E981` (file offset `0x13DD81`).

Bytes en el caller: `E8 5A D5 FF FF` → `call 0x53BEE0` ✓

Contexto del caller (0x53E920–0x53E990): es la función del bucle idle del juego, que ejecuta la lógica de un frame completo. La llamada a `CGame::Process` aparece tras `C7 05 D0 F0 C3 00 00 00 00 00` (escritura de un contador de estado).

### Referencia cruzada con documentación pública
plugin-sdk (GTA SA 1.0 US) declara `CGame::Process` en `0x53BEE0`. Confirmado contra el binario real: **coincide**.

### Validación del hook
El hook de este mod reemplaza `E8 5A D5 FF FF` en `0x53E981` por `E8 <rel32 → on_game_process>`.  
`on_game_process` llama primero a `0x53BEE0` (trampoline) y después a `tick()`.  
Se registra en log: bytes antes y después del parche.
