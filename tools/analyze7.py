#!/usr/bin/env python3
"""
analyze7.py – Determinar definitivamente la cadena de punteros desde
0xB74490 hasta el ped con offset 0x540.

Hipótesis final basada en plugin-sdk:
  CWorld::Players[0] está en 0xB74490.
  CPlayerInfo::m_pPed es el PRIMER campo (offset 0x00) de CPlayerInfo.
  PERO CPlayerInfo en SA tiene:
    [+0x00] puntero al CPed
  Así: ped = *(*(0xB74490)) -- doble deref.

ALTERNATIVA (también en plugin-sdk):
  0xB74490 es array de CPlayerInfo INLINE (no punteros).
  CPlayerInfo[0].m_pPed está en offset 0x00 de CPlayerInfo.
  Así: ped = *(0xB74490) -- una sola deref.

Mi código actual hace: players_ptr.read() → una deref.
Si eso da 0.0 de salud, puede ser:
  1. El ped era nulo en ese momento (pantalla de carga/menú)
  2. La dirección del ped es correcta pero los offsets 0x540/0x544 son incorrectos
  3. Hay una doble deref necesaria

Estrategia: buscar el código que ESCRIBE salud=100 en el cheat HESOYAM.
Esa función debe: (1) obtener el ped, (2) escribir 100.0 a [ped+offset_health].
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

def read_va(va, n=128):
    off = va_to_off(va)
    return data[off:off+n] if off is not None else None

def hexb(bs, n=None):
    if bs is None: return "N/A"
    bs = bs[:n] if n else bs
    return ' '.join(f'{b:02X}' for b in bs)

PLAYERS = 0xB74490

# ── 1. Buscar el cheat handler de HESOYAM (salud+armadura+dinero) ────────────
print("=" * 70)
print("1. Buscar cheat HESOYAM handler (escribe 100.0 y 150.0 de salud)")
print("=" * 70)
# HESOYAM sets health to 100.0 (0x42C80000) and armor to 100.0
# Float 100.0 = 0x42C80000
# Float 150.0 = 0x43160000
# Search for PUSH 0x42C80000 or MOV [reg], 0x42C80000 near 0xB74490 access
val_100f = struct.pack("<f", 100.0)
val_150f = struct.pack("<f", 150.0)
print(f"  100.0f = {' '.join(f'{b:02X}' for b in val_100f)}")
print(f"  150.0f = {' '.join(f'{b:02X}' for b in val_150f)}")

# Search for FLD [const_100] or MOV [reg+offset], 100.0 patterns
# Pattern: C7 86 xx xx 00 00 00 00 C8 42 (MOV [ESI+offset], 100.0 as DWORD)
# or: D9 05 xx xx xx xx (FLD [mem]) where mem contains 100.0

# 100.0 as bytes 00 00 C8 42 — search for this float value being stored
PAT_100 = bytes([0x00, 0x00, 0xC8, 0x42])
hits_100 = []
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_100, pos)
        if idx == -1: break
        va = BASE + vaddr + idx
        ctx = seg[max(0,idx-6):idx+8]
        # Check if preceded by C7 86/87/80/81/82/83/84/85 (MOV [reg+disp32], imm32)
        # or by C7 05 (MOV [mem32], imm32)
        b_m6 = seg[idx-6] if idx >= 6 else 0
        b_m5 = seg[idx-5] if idx >= 5 else 0
        b_m2 = seg[idx-2] if idx >= 2 else 0
        b_m1 = seg[idx-1] if idx >= 1 else 0
        if b_m2 == 0xC7 and (b_m1 & 0xC0) in (0x80, 0x40):
            hits_100.append((va-2, ctx, 'MOV [reg+disp], 100.0'))
        elif b_m6 == 0xC7 and b_m5 == 0x05:
            hits_100.append((va-6, ctx, 'MOV [mem32], 100.0'))
        pos = idx + 1

print(f"\n  MOV [..], 100.0f patterns: {len(hits_100)}")
for va, ctx, desc in hits_100[:15]:
    print(f"    0x{va:08X}: {hexb(ctx)}  ({desc})")

# ── 2. Leer la función 0x439B20 (referenciada en tabla de cheats) ─────────────
print()
print("=" * 70)
print("2. Función 0x439B20 (en tabla de cheats en 0x8A5B8C)")
print("=" * 70)
# From analyze3: 0x8A5B8C: hash=0x00438FC0  val=0x00439B20
# This might be a cheat handler. Read it.
bs = read_va(0x439B20, 128)
print(f"  Bytes at 0x439B20:")
for row in range(0, 96, 16):
    print(f"    0x{0x439B20+row:08X}: {hexb(bs[row:row+16])}")

# ── 3. Leer la función 0x438F70, 0x438F80 (también en la tabla) ─────────────
print()
print("=" * 70)
print("3. Funciones 0x438F70 y 0x438F80 (tabla de cheats)")
print("=" * 70)
for va_fn in [0x438F70, 0x438F80, 0x438F90, 0x438FC0]:
    bs = read_va(va_fn, 48)
    print(f"  0x{va_fn:08X}: {hexb(bs, 32)}")

# ── 4. Función 0x439B20 – buscar acceso a 0xB74490 dentro de ella ────────────
print()
print("=" * 70)
print("4. ¿Accede 0x439B20 a CWorld::Players?")
print("=" * 70)
PAT_PLAYERS = struct.pack("<I", PLAYERS)
bs_439 = read_va(0x439B20, 256)
if bs_439:
    idx = bs_439.find(PAT_PLAYERS)
    if idx != -1:
        print(f"  Acceso a 0xB74490 en offset +{idx} dentro de 0x439B20")
        ctx = bs_439[max(0,idx-4):idx+8]
        print(f"  Ctx: {hexb(ctx)}")
    else:
        print(f"  NO accede directamente a 0xB74490")
        # It might call a helper that does
        # Find all CALL targets
        for i in range(len(bs_439)-5):
            if bs_439[i] == 0xE8:
                rel = struct.unpack_from("<i", bs_439, i+1)[0]
                tgt = 0x439B20 + i + 5 + rel
                print(f"  CALL @ +{i} → 0x{tgt:08X}")

# ── 5. DECISIVO: leer función que escribe [reg+0x540] con 0xB74490 near ──────
print()
print("=" * 70)
print("5. Función 0x47EA39 – decodificación completa")
print("=" * 70)
# 0x47EA39: MOV ECX,[0xB74490]; CALL xxx; FLD [EAX+0x540]
# Read from 0x47EA30 for context
bs = read_va(0x47EA30, 80)
print(f"  Bytes at 0x47EA30:")
for row in range(0, 64, 16):
    print(f"    0x{0x47EA30+row:08X}: {hexb(bs[row:row+16])}")

# Decode the CALL target
off = va_to_off(0x47EA39)
if off:
    # Find E8 near this offset
    for i in range(0, 20):
        if data[off + i] == 0xE8:
            rel = struct.unpack_from("<i", data, off+i+1)[0]
            tgt = 0x47EA39 + i + 5 + rel
            print(f"\n  CALL at 0x{0x47EA39+i:08X} → 0x{tgt:08X}")
            bs_tgt = read_va(tgt, 32)
            print(f"  Called function @ 0x{tgt:08X}: {hexb(bs_tgt, 16)}")
            break

# ── 6. DECISIVO: buscar la función que hace *(*(0xB74490)) vs *(0xB74490) ────
print()
print("=" * 70)
print("6. ¿Es 0xB74490 puntero al ped, o puntero a struct con ped en [+N]?")
print("=" * 70)
print("Buscando: MOV EAX,[0xB74490]; (sin call) luego D9 80 40 05 00 00 (FLD [EAX+0x540])")

PAT_A1   = bytes([0xA1, 0x90, 0x44, 0xB7, 0x00])
PAT_FLD  = bytes([0xD9, 0x80, 0x40, 0x05, 0x00, 0x00])  # FLD [EAX+0x540]
PAT_FLD2 = bytes([0xD9, 0x86, 0x40, 0x05, 0x00, 0x00])  # FLD [ESI+0x540]
PAT_FLD3 = bytes([0xD9, 0x87, 0x40, 0x05, 0x00, 0x00])  # FLD [EDI+0x540]
PAT_MOV1 = bytes([0x8B, 0x80, 0x40, 0x05, 0x00, 0x00])  # MOV EAX,[EAX+0x540]
PAT_MOV2 = bytes([0x89, 0x86, 0x40, 0x05, 0x00, 0x00])  # MOV [ESI+0x540],EAX (write)
PAT_MOV3 = bytes([0x89, 0x88, 0x40, 0x05, 0x00, 0x00])  # MOV [EAX+0x540],ECX (write)

for desc, pat in [("FLD [EAX+0x540]",PAT_FLD),("FLD [ESI+0x540]",PAT_FLD2),
                   ("FLD [EDI+0x540]",PAT_FLD3),("MOV EAX,[EAX+0x540]",PAT_MOV1),
                   ("MOV [ESI+0x540],EAX (write)",PAT_MOV2),
                   ("MOV [EAX+0x540],ECX (write)",PAT_MOV3)]:
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sec_name and 'TEXT' not in sec_name: continue
        seg = data[raw_off:raw_off+raw_size]
        pos = 0
        while True:
            idx = seg.find(pat, pos)
            if idx == -1: break
            # Look BACK for [0xB74490] access within 150 bytes
            back = seg[max(0,idx-150):idx]
            # Check for A1 90 44 B7 00
            bidx = back.rfind(bytes([0x90, 0x44, 0xB7, 0x00]))
            if bidx != -1:
                va_load = BASE + vaddr + (idx - 150 + bidx - 1)  # rough
                va_use  = BASE + vaddr + idx
                print(f"  {desc} @ 0x{va_use:08X}")
                ctx = seg[idx-10:idx+16]
                print(f"    ctx: {hexb(ctx)}")
                # Check the register used in the [reg+0x540] instruction
                # and compare with what was loaded from [0xB74490]
                reg_load_byte = back[bidx-1] if bidx > 0 else 0
                reg_use_byte  = pat[1]
                print(f"    Load modrm=0x{reg_load_byte:02X}, use modrm=0x{reg_use_byte:02X}")
                if reg_load_byte == reg_use_byte:
                    print(f"    → MISMO REGISTRO: [0xB74490] → ped_ptr → [ped+0x540]")
                    print(f"    → CONFIRMADO: una sola deref es suficiente")
            pos = idx + 1

print()
print("=" * 70)
print("Análisis 7 completado.")
print("=" * 70)
