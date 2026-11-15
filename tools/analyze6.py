#!/usr/bin/env python3
"""
analyze6.py – Confirmar la estructura de CPlayerInfo y el offset del ped.

Patrón dominante visto:
  MOV EAX, [0xB74490]   ← EAX = *(0xB74490) = ¿dirección de CPlayerInfo? ¿o el ped?
  MOV EDI, [EAX+0x08]   ← accede a EAX+0x08

Hipótesis 1: 0xB74490 contiene un PUNTERO a CPlayerInfo
  → *(0xB74490) = dirección de CPlayerInfo struct
  → [CPlayerInfo + 0x08] = puntero al CPed

Hipótesis 2: 0xB74490 ES la CPlayerInfo struct inline
  → code uses [0xB74490] como campo, luego [0xB74498] como otro campo
  → uno de esos campos es el puntero al CPed

La evidencia inline de 0xB74490+offset confirma hipótesis 2.
Pero... ¿el plugin-sdk documenta CPlayerInfo::m_pPed a offset 0x00?
En esa hipótesis: *(*(0xB74490)) = puntero al CPed.
Eso sería: EAX = *(0xB74490) = ped_ptr, luego [EAX+0x08] = algo DEL ped.

Vamos a leer el contexto de xref 3 para ver qué hace con EAX+0x08:
  0x00409D89: MOV EAX, [0xB74490]; MOV EDI, [EAX+0x08]
              después: 85 FF 74 5D → TEST EDI,EDI  JZ ...
              luego: 8B DF 69 DB C4 07 00 00 → MOV EBX,EDI; IMUL EBX,EBX,0x7C4
              → EBX = EDI * 0x7C4 (esto es un índice multiplicado por tamaño = puntero a array)
              → EDI es un ÍNDICE, no un puntero. Esto no es el ped.

Hipótesis revisada:
  0xB74490 contiene el primer campo de CPlayerInfo array.
  MOV EAX, [0xB74490] → EAX = primer campo (podría ser puntero al ped en offset 0)
  MOV EDI, [EAX+0x08] → si EAX es puntero al ped, [ped+0x08] es algo del ped (¿vehicle ptr?)

Vamos a confirmar: en plugin-sdk, CPlayerInfo[0] está a 0xB74490,
y el puntero al ped es el PRIMER campo (offset 0x00) de CPlayerInfo.
Así: ped = *(0xB74490) = el valor almacenado en 0xB74490.
Luego [ped + 0x540] = health.

PERO: health = 0.0 y max_health = 0.0 en el juego, lo que significa
que el puntero que obtenemos no apunta a un ped válido.

Posible causa: el jugador no tiene ped activo en ese momento
(en el menú principal, en la pantalla de carga, etc.)
O: el puntero está en un offset diferente.
"""

import struct
from pathlib import Path

EXE  = r"C:\Users\Ericson\rustyscript\work\gta_sa.exe"
BASE = 0x400000
data = Path(EXE).read_bytes()

pe_off  = struct.unpack_from("<I", data, 0x3C)[0]
num_sec = struct.unpack_from("<H", data, pe_off + 6)[0]
opt_sz  = struct.unpack_from("<H", data, pe_off + 20)[0]
sec_off = pe_off + 24 + opt_sz
sections = []
for i in range(num_sec):
    b = sec_off + i * 40
    name     = data[b:b+8].rstrip(b'\x00').decode('ascii', errors='replace')
    vsize    = struct.unpack_from("<I", data, b+8)[0]
    vaddr    = struct.unpack_from("<I", data, b+12)[0]
    raw_size = struct.unpack_from("<I", data, b+16)[0]
    raw_off  = struct.unpack_from("<I", data, b+20)[0]
    sections.append((name, vaddr, vsize, raw_off, raw_size))

def va_to_off(va):
    rva = va - BASE
    for (n, vaddr, vsize, raw_off, raw_size) in sections:
        if vaddr <= rva < vaddr + max(vsize, raw_size):
            return raw_off + (rva - vaddr)
    return None

def read_va(va, n=64):
    off = va_to_off(va)
    return data[off:off+n] if off is not None else None

def hexb(bs, n=None):
    if bs is None: return "N/A"
    bs = bs[:n] if n else bs
    return ' '.join(f'{b:02X}' for b in bs)

PLAYERS = 0xB74490

# ── Leer el xref en 0x409D89 con contexto completo ──────────────────────────
print("=" * 70)
print("Contexto completo de 0x409D89 (acceso a [0xB74490] luego [EAX+0x08])")
print("=" * 70)
bs = read_va(0x409D80, 80)
print(hexb(bs, 48))
print()
# Decode manually:
# 00409D80: ... A1 90 44 B7 00    MOV EAX, [0xB74490]
# 00409D85: 8B 78 08              MOV EDI, [EAX+8]
# 00409D88: 85 FF                 TEST EDI, EDI
# 00409D8A: 74 5D                 JZ +0x5D
# 00409D8C: 8B DF                 MOV EBX, EDI
# 00409D8E: 69 DB C4 07 00 00     IMUL EBX, EBX, 0x7C4
# → EBX = EDI * 0x7C4 = EDI * 1988
# 0x7C4 = 1988 bytes = size of a CEntity? or CVehicle?
# This suggests EDI is an INDEX into an array, not a pointer.
# [EAX+0x08] = index of current player ped in some array? Or vehicle index?
print("Decodificación manual:")
print("  MOV EAX, [0xB74490]   → EAX = *(0xB74490)")
print("  MOV EDI, [EAX+0x08]   → EDI = *(EAX + 8)")
print("  TEST EDI,EDI; JZ ...  → null check")
print("  IMUL EBX, EDI, 0x7C4  → EBX = EDI * 1988")
print("  → 0x7C4 = 1988: podría ser tamaño de CVehicle struct")
print("  → EDI es probablemente un ÍNDICE de vehículo, no puntero al ped")
print()
print("  CONCLUSIÓN: 0xB74490 contiene el PUNTERO AL PED directamente (EAX = ped_ptr)")
print("  y [ped+0x08] = índice del vehículo actual del jugador.")

# ── Verificar con la función que escribe health ──────────────────────────────
print()
print("=" * 70)
print("Función 0x444E59 (escribe health/maxhealth) — cómo obtiene el ped")
print("=" * 70)
# Esta función fue identificada como la que escribe [ESI+0x540] y [ESI+0x544]
# ¿Cómo llega ESI al puntero al ped?
bs = read_va(0x444E59, 128)
print(f"Bytes 0x444E59 (96 bytes):")
for row in range(0, 96, 16):
    print(f"  0x{0x444E59+row:08X}: {hexb(bs[row:row+16])}")

# ── Buscar funciones que leen [0xB74490] y luego acceden a offset 0x540 ───
print()
print("=" * 70)
print("Funciones que leen [0xB74490] y luego [EAX+0x540] (ped health)")
print("=" * 70)
PAT_LOAD = bytes([0xA1, 0x90, 0x44, 0xB7, 0x00])  # MOV EAX, [0xB74490]
PAT_HEALTH = bytes([0x40, 0x05, 0x00, 0x00])       # offset 0x540

# Buscar funciones donde ambos patrones aparecen cerca
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_LOAD, pos)
        if idx == -1: break
        # Look for health offset within 200 bytes after
        window = seg[idx:idx+200]
        hidx = window.find(PAT_HEALTH)
        if hidx != -1:
            va = BASE + vaddr + idx
            va_h = va + hidx
            print(f"  MOV EAX,[0xB74490] @ 0x{va:08X}, then [+0x540] @ 0x{va_h:08X}")
            ctx = seg[idx:idx+hidx+8]
            print(f"  Full context: {hexb(ctx[:32])}")
            print(f"               {hexb(ctx[32:64]) if len(ctx)>32 else ''}")
        pos = idx + 1

# ── Buscar el patrón: cargar [0xB74490], luego usar como ped base ────────────
print()
print("=" * 70)
print("Patrón: MOV reg, [0xB74490]; ... [reg+0x540] o [reg+0x548]")
print("=" * 70)
# Pattern: 8B xx 90 44 B7 00 (MOV reg, [0xB74490])
# followed within 50 bytes by: xx xx 40 05 00 00 or xx xx 48 05 00 00
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    # search for 8B xx 90 44 B7 00
    for i in range(len(seg) - 60):
        if seg[i] == 0x8B and seg[i+2:i+6] == bytes([0x90, 0x44, 0xB7, 0x00]):
            modrm = seg[i+1]
            # Check it's a MOV reg,[mem] (mod=00, rm=101=disp32)
            if (modrm & 0xC7) == 0x05:
                reg = (modrm >> 3) & 7
                regs = ['EAX','ECX','EDX','EBX','ESP','EBP','ESI','EDI']
                reg_name = regs[reg]
                # Look for [reg+0x540] within next 100 bytes
                # ModRM for [reg+disp32]: mod=10, rm=reg → 0x80 | (reg<<3) | reg (but that's wrong)
                # Actually: [ESI+0x540] = ModRM 0x86, disp32 0x540
                # ModRM byte for [regN + disp32]: mod=10 (0x80), reg=dest, r/m=src_reg
                # We look for the disp32 pattern 40 05 00 00
                window = seg[i+6:i+6+100]
                hidx = window.find(bytes([0x40, 0x05, 0x00, 0x00]))
                if hidx != -1:
                    va = BASE + vaddr + i
                    print(f"  MOV {reg_name},[0xB74490] @ 0x{va:08X}, then [+0x540] +{hidx+6} bytes later")
                    ctx = seg[i:i+hidx+6+8]
                    print(f"    {hexb(ctx[:24])}")

# ── Verificar con 8B 3D pattern (MOV EDI, [0xB74490]) ───────────────────────
print()
print("=" * 70)
print("MOV EDI,[0xB74490] luego [EDI+0x540]")
print("=" * 70)
PAT_EDI = bytes([0x8B, 0x3D, 0x90, 0x44, 0xB7, 0x00])
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_EDI, pos)
        if idx == -1: break
        window = seg[idx:idx+200]
        hidx = window.find(bytes([0x40, 0x05, 0x00, 0x00]))
        if hidx != -1:
            va = BASE + vaddr + idx
            print(f"  MOV EDI,[0xB74490] @ 0x{va:08X}")
            ctx = seg[idx:idx+hidx+8]
            print(f"    {hexb(ctx[:32])}")
        pos = idx + 1

print()
print("=" * 70)
print("Análisis 6 completado.")
print("=" * 70)
