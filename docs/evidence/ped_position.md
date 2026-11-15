# CPed position offsets (CPlaceable::CMatrix)

| Campo               | Valor                                               |
|---------------------|-----------------------------------------------------|
| Offset X            | `+0x44` (f32)                                       |
| Offset Y            | `+0x48` (f32)                                       |
| Offset Z            | `+0x4C` (f32)                                       |
| Nivel de confianza  | **ALTA**                                            |
| Prueba en juego     | Pendiente                                           |

## Evidencia

### CPlaceable::GetPosition @ 0x404460
```
8D 41 44    LEA EAX, [ECX+0x44]
C3          RET
```
Función de 4 bytes: retorna un puntero al campo `+0x44` del objeto.
Usada con `ECX = ped_ptr` en código de movimiento de NPCs y jugador.

### Accesos FLD/FSTP a [reg+0x44]
Búsqueda de `D9 40 44` (FLD [EAX+0x44]): 5 hits en .text, todos en contextos
donde X, Y, Z se leen juntos (`+0x44`, `+0x48`, `+0x4C`):
```
0x407F53: D9 40 44 D9 40 48 D9 40 4C  → FLD X; FLD Y; FLD Z
0x40B677: D9 40 48 D9 40 44 ...        → acceso a los tres juntos
```
Patrón consistente con `CMatrix.pos.x/y/z`.

### SetPosn @ 0x4A50C1
```
8B 4C 24 04    MOV ECX, [ESP+4]      ; this ptr
D9 58 44       FSTP [EAX+0x44]       ; store X
8B 54 24 08    MOV EDX, [ESP+8]
89 48 48       MOV [EAX+0x48], ECX   ; store Y
89 50 4C       MOV [EAX+0x4C], EDX   ; store Z
```
Escritura directa a los tres offsets consecutivos.

### Lectura en Rust
```rust
let ped: *mut u8 = get_ped();
let x = (ped.add(0x44) as *const f32).read();
let y = (ped.add(0x48) as *const f32).read();
let z = (ped.add(0x4C) as *const f32).read();
```
