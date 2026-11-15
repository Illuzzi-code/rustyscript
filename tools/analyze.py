#!/usr/bin/env python3
"""
analyze.py  –  Binary analysis tool for GTA SA 1.0 US (gta_sa.exe)
Produces:
  - SHA-256 hash + size verification
  - Byte signatures at known VAs
  - Pattern searches (prologues, strings, xrefs)
  - Cheat hash table location
  - CGame::Process caller search
Usage: python analyze.py <path_to_gta_sa.exe>
"""

import sys
import hashlib
import struct
import os
from pathlib import Path

EXE_PATH = sys.argv[1] if len(sys.argv) > 1 else r"C:\Users\Ericson\rustyscript\work\gta_sa.exe"
IMAGE_BASE = 0x400000

KNOWN_SHA256 = "F01A00CE950FA40CA1ED59DF0E789848C6EDCF6405456274965885D0929343AC"
KNOWN_SIZE   = 14383616

data = Path(EXE_PATH).read_bytes()
size = len(data)
sha  = hashlib.sha256(data).hexdigest().upper()

print("=" * 70)
print("FASE 0 — Verificación del ejecutable")
print("=" * 70)
print(f"  Archivo : {EXE_PATH}")
print(f"  Tamaño  : {size} bytes  (esperado: {KNOWN_SIZE})  {'OK' if size==KNOWN_SIZE else 'DIFERENTE'}")
print(f"  SHA-256 : {sha}")
print(f"            {'OK – GTA SA 1.0 US confirmado' if sha==KNOWN_SHA256 else 'NO COINCIDE con 1.0 US'}")

# ── PE parsing ──────────────────────────────────────────────────────────────
pe_off  = struct.unpack_from("<I", data, 0x3C)[0]
machine = struct.unpack_from("<H", data, pe_off + 4)[0]
num_sec = struct.unpack_from("<H", data, pe_off + 6)[0]
magic   = struct.unpack_from("<H", data, pe_off + 24)[0]
ep_rva  = struct.unpack_from("<I", data, pe_off + 40)[0]
img_base_pe = struct.unpack_from("<I", data, pe_off + 52)[0]
print(f"\nPE Header @ 0x{pe_off:X}")
print(f"  Machine     : 0x{machine:04X}  ({'i386' if machine==0x14C else '?'})")
print(f"  Sections    : {num_sec}")
print(f"  Magic       : 0x{magic:04X}  ({'PE32' if magic==0x10B else '?'})")
print(f"  EntryPoint  : 0x{ep_rva + img_base_pe:08X}")
print(f"  ImageBase   : 0x{img_base_pe:08X}")

# ── Helper: VA → file offset ─────────────────────────────────────────────────
# Parse section table to map RVA → file offset properly
sec_hdr_off = pe_off + 24 + struct.unpack_from("<H", data, pe_off + 20)[0]
sections = []
for i in range(num_sec):
    base = sec_hdr_off + i * 40
    name      = data[base:base+8].rstrip(b'\x00').decode('ascii', errors='replace')
    vsize     = struct.unpack_from("<I", data, base + 8)[0]
    vaddr     = struct.unpack_from("<I", data, base + 12)[0]
    raw_size  = struct.unpack_from("<I", data, base + 16)[0]
    raw_off   = struct.unpack_from("<I", data, base + 20)[0]
    sections.append((name, vaddr, vsize, raw_off, raw_size))

def va_to_off(va):
    rva = va - IMAGE_BASE
    for (name, vaddr, vsize, raw_off, raw_size) in sections:
        if vaddr <= rva < vaddr + max(vsize, raw_size):
            return raw_off + (rva - vaddr)
    return None

def read_va(va, n=16):
    off = va_to_off(va)
    if off is None:
        return None
    return data[off:off+n]

def hexstr(bs):
    return ' '.join(f'{b:02X}' for b in bs) if bs else 'N/A'

print("\nSecciones:")
for s in sections:
    print(f"  {s[0]:8s}  VAddr=0x{s[1]:08X}  VSize=0x{s[2]:08X}  RawOff=0x{s[3]:08X}  RawSize=0x{s[4]:08X}")

# ── Known addresses from log ─────────────────────────────────────────────────
print("\n" + "=" * 70)
print("Firmas de bytes en direcciones del log")
print("=" * 70)

candidates = [
    ("CGame::Process",       0x53BEE0, "log dice: 83 EC 0C 53 56 57 E8 E5"),
    ("AddMessageJumpQ",      0x69F1E0, "log dice: 81 EC 20 03 00 00 56 8B"),
    ("SetHelpMessage",       0x588BE0, "log dice: A0 C0 AE BA 00 84 C0 0F"),
    ("VehicleCheat(BAD)",    0x439390, "log dice: 68 00 00 00 C7 44 24 10"),
]

for name, va, note in candidates:
    bs = read_va(va, 16)
    off = va_to_off(va)
    off_str = f"0x{off:X}" if off is not None else "N/A"
    print(f"\n  {name} @ 0x{va:08X}  (file off {off_str})")
    print(f"    Bytes  : {hexstr(bs)}")
    print(f"    Note   : {note}")

# ── Search for the REAL CGame::Process ──────────────────────────────────────
# Log says prologue is [83 EC 0C 53 56 57 E8 E5] – search for it
print("\n" + "=" * 70)
print("Búsqueda de CGame::Process prologue: 83 EC 0C 53 56 57")
print("=" * 70)

TARGET = bytes([0x83, 0xEC, 0x0C, 0x53, 0x56, 0x57])
hits = []
for sec in sections:
    sname, vaddr, vsize, raw_off, raw_size = sec
    if sname not in ('.text', 'CODE', '.code'):
        continue
    seg = data[raw_off:raw_off+raw_size]
    pos = 0
    while True:
        idx = seg.find(TARGET, pos)
        if idx == -1:
            break
        va = IMAGE_BASE + vaddr + idx
        ctx = seg[idx:idx+16]
        hits.append((va, ctx))
        pos = idx + 1

print(f"  Hits en .text: {len(hits)}")
for va, ctx in hits[:20]:
    print(f"    0x{va:08X}: {hexstr(ctx)}")

# ── Search for CALL to CGame::Process ───────────────────────────────────────
# If we find CGame::Process, search for E8 xx xx xx xx that calls it
if hits:
    cgame_va = hits[0][0]
    print(f"\n  → Usando CGame::Process @ 0x{cgame_va:08X}")
    print(f"\nBúsqueda de CALL E8 → 0x{cgame_va:08X}")
    callers = []
    for sec in sections:
        sname, vaddr, vsize, raw_off, raw_size = sec
        if sname not in ('.text', 'CODE', '.code'):
            continue
        seg = data[raw_off:raw_off+raw_size]
        for i in range(len(seg) - 5):
            if seg[i] == 0xE8:
                rel = struct.unpack_from("<i", seg, i+1)[0]
                call_va = IMAGE_BASE + vaddr + i
                target_va = call_va + 5 + rel
                if target_va == cgame_va:
                    ctx = seg[i:i+16]
                    callers.append((call_va, ctx))
    print(f"  Callers encontrados: {len(callers)}")
    for va, ctx in callers[:10]:
        print(f"    0x{va:08X}: {hexstr(ctx)}")

# ── Search for the REAL AddMessageJumpQ ─────────────────────────────────────
# Log says: 81 EC 20 03 00 00 56 8B  → sub esp, 0x320 ; push esi
print("\n" + "=" * 70)
print("Búsqueda de AddMessageJumpQ: 81 EC 20 03 00 00")
print("=" * 70)
PAT_MSG = bytes([0x81, 0xEC, 0x20, 0x03, 0x00, 0x00])
for sec in sections:
    sname, vaddr, vsize, raw_off, raw_size = sec
    if sname not in ('.text', 'CODE', '.code'):
        continue
    seg = data[raw_off:raw_off+raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_MSG, pos)
        if idx == -1:
            break
        va = IMAGE_BASE + vaddr + idx
        ctx = seg[idx:idx+16]
        print(f"    0x{va:08X}: {hexstr(ctx)}")
        pos = idx + 1

# ── Search for SetHelpMessage ────────────────────────────────────────────────
# Log says: A0 C0 AE BA 00 84 C0 0F  – this looks like it's reading from a 
# global flag address. Let's search for a known prologue near this.
# SetHelpMessage in plugin-sdk for 1.0 is @ 0x58A330. Let's check that too.
print("\n" + "=" * 70)
print("Verificando SetHelpMessage @ 0x58A330 (plugin-sdk 1.0 US)")
print("=" * 70)
bs = read_va(0x58A330, 16)
print(f"  0x58A330: {hexstr(bs)}")
# Also check 0x588BE0
bs2 = read_va(0x588BE0, 16)
print(f"  0x588BE0: {hexstr(bs2)}")

# ── Cheat hash table search ──────────────────────────────────────────────────
# CCheat in GTA SA processes cheats via a hash table.
# Known cheat handler strings: "HESOYAM", "UZUMYMW", "ROCKETMAN" etc.
# Search for ASCII "HESOYAM" in .data section
print("\n" + "=" * 70)
print("Búsqueda de strings de cheats (HESOYAM, UZUMYMW, ROCKETMAN)")
print("=" * 70)
cheat_strings = [b"HESOYAM", b"UZUMYMW", b"ROCKETMAN", b"JUMPJET", b"RIPAZHA",
                 b"AIYPWZQP", b"OLDSPEEDDEMON"]
for cs in cheat_strings:
    idx = data.find(cs)
    if idx != -1:
        va = IMAGE_BASE + idx  # rough, might be in data segment
        print(f"  '{cs.decode()}' @ file 0x{idx:X}  (rough VA 0x{va:08X})")
    else:
        print(f"  '{cs.decode()}' NOT FOUND (might be hashed)")

# ── Search for cheat hash values ─────────────────────────────────────────────
# GTA SA uses CRC32-like hashes of cheat strings stored in a table.
# Known hash for HESOYAM (health/money/armor) = 0x5B5EDB3D
# ROCKETMAN (jetpack) = 0x0D5C1F23 (these are from plugin-sdk / gtamods wiki)
# Let's search for these dword values in .text/.data
print("\n" + "=" * 70)
print("Búsqueda de hash conocidos de cheats en el binario")
print("=" * 70)
# These hashes come from plugin-sdk / gtamods – they are HYPOTHESES to confirm
cheat_hashes = {
    "HESOYAM (health/money/armor)": 0x5B5EDB3D,
    "ROCKETMAN (jetpack)":          0x0D5C1F23,
    "UZUMYMW (weapon set 1)":       0x2E8F84D8,
    "RIPAZHA (super jump)":         0xD44B34CF,
    "JUMPJET (hydra)":              0xF0CA83AC,
    "AIYPWZQP (spawn Rhino)":       0x89B5B1A3,
}
for name, h in cheat_hashes.items():
    hb = struct.pack("<I", h)
    idx = data.find(hb)
    if idx != -1:
        # Check surrounding context (is this inside a table?)
        ctx = data[idx-4:idx+12]
        print(f"  {name}")
        print(f"    Hash 0x{h:08X} @ file 0x{idx:X}  ctx: {hexstr(ctx)}")
    else:
        print(f"  {name}  Hash 0x{h:08X} NOT FOUND")

# ── CWorld::Players ──────────────────────────────────────────────────────────
# In GTA SA 1.0 US, CWorld::Players is at 0xB74490 (plugin-sdk)
# This is in BSS/data, so it won't be in the file directly.
# But we can search for references to this address in .text
print("\n" + "=" * 70)
print("Búsqueda de referencias a CWorld::Players (0xB74490)")
print("=" * 70)
target_bytes = struct.pack("<I", 0xB74490)
hits = []
for sec in sections:
    sname, vaddr, vsize, raw_off, raw_size = sec
    if sname not in ('.text', 'CODE', '.code'):
        continue
    seg = data[raw_off:raw_off+raw_size]
    pos = 0
    while True:
        idx = seg.find(target_bytes, pos)
        if idx == -1:
            break
        va = IMAGE_BASE + vaddr + idx
        ctx = seg[max(0,idx-4):idx+8]
        hits.append((va, ctx))
        pos = idx + 1
hits.sort(key=lambda x: x[0])
print(f"  Referencias a 0xB74490: {len(hits)}")
for va, ctx in hits[:15]:
    print(f"    0x{va:08X}: {hexstr(ctx)}")

# ── CGame::Process – also check plugin-sdk canonical address ─────────────────
print("\n" + "=" * 70)
print("Verificando CGame::Process @ 0x53BEE0 con sección correcta")
print("=" * 70)
off53 = va_to_off(0x53BEE0)
if off53:
    bs = data[off53:off53+32]
    print(f"  file offset 0x{off53:X}")
    print(f"  bytes: {hexstr(bs[:16])}")
    print(f"         {hexstr(bs[16:32])}")
    # Check if this looks like a function: typical prologue opcodes
    # 55 = push ebp, 53 = push ebx, 56 = push esi, 57 = push edi
    # 83 EC xx = sub esp, xx    81 EC xx = sub esp (large)
    first = bs[0]
    print(f"  First opcode: 0x{first:02X}  → {'push ebp' if first==0x55 else 'sub esp,N' if first==0x83 else 'other'}")
else:
    print("  VA outside mapped sections")

print("\n" + "=" * 70)
print("Análisis completado.")
print("=" * 70)
