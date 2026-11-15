#!/usr/bin/env python3
"""
analyze2.py – Análisis profundo de direcciones específicas:
  1. Confirmar CGame::Process @ 0x53BEE0 y encontrar su único caller
  2. Analizar 0x588BE0 (SetHelpMessage real)
  3. Encontrar CGame::Process caller buscando E8 correctamente en TODAS las secciones .text
  4. Analizar 0x439390 y zona adyacente para entender qué hay allí
  5. Buscar tabla de cheats via patrón estructural (array de structs hash+fnptr)
  6. CWorld::Players offset dentro del ped struct (ped health/armor offsets)
"""

import sys
import struct
from pathlib import Path

EXE  = r"C:\Users\Ericson\rustyscript\work\gta_sa.exe"
BASE = 0x400000
data = Path(EXE).read_bytes()

# ── PE section map ───────────────────────────────────────────────────────────
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
    for (name, vaddr, vsize, raw_off, raw_size) in sections:
        if vaddr <= rva < vaddr + max(vsize, raw_size):
            return raw_off + (rva - vaddr)
    return None

def read_va(va, n=32):
    off = va_to_off(va)
    return data[off:off+n] if off is not None else None

def hexb(bs, n=None):
    if bs is None: return "N/A"
    if n: bs = bs[:n]
    return ' '.join(f'{b:02X}' for b in bs)

def disasm_simple(va, n=48):
    """Very simple linear disassembler for common x86 patterns, enough to see call targets."""
    bs = read_va(va, n)
    if not bs: return
    i = 0
    cur_va = va
    while i < len(bs) - 5:
        b = bs[i]
        if b == 0xE8:  # CALL rel32
            rel = struct.unpack_from("<i", bs, i+1)[0]
            tgt = cur_va + 5 + rel
            print(f"    0x{cur_va:08X}  E8 {rel:+09X}  CALL 0x{tgt:08X}")
            i += 5; cur_va += 5
        elif b == 0x55:
            print(f"    0x{cur_va:08X}  55            PUSH EBP")
            i += 1; cur_va += 1
        elif b == 0x83 and i+2 < len(bs):
            print(f"    0x{cur_va:08X}  {bs[i]:02X} {bs[i+1]:02X} {bs[i+2]:02X}       SUB/CMP/...")
            i += 3; cur_va += 3
        elif b == 0xC3:
            print(f"    0x{cur_va:08X}  C3            RET")
            break
        elif b == 0xC2:
            n = struct.unpack_from("<H", bs, i+1)[0]
            print(f"    0x{cur_va:08X}  C2 {n:04X}      RET {n}")
            break
        else:
            print(f"    0x{cur_va:08X}  {bs[i]:02X} ..")
            i += 1; cur_va += 1

# ═══════════════════════════════════════════════════════════════════════════════
print("=" * 70)
print("1. CGame::Process @ 0x53BEE0 – callers en TODO el ejecutable")
print("=" * 70)

CGAME = 0x53BEE0
callers = []
for name, vaddr, vsize, raw_off, raw_size in sections:
    seg = data[raw_off:raw_off + raw_size]
    for i in range(len(seg) - 5):
        if seg[i] == 0xE8:
            rel = struct.unpack_from("<i", seg, i+1)[0]
            call_va  = BASE + vaddr + i
            tgt_va   = call_va + 5 + rel
            if tgt_va == CGAME:
                ctx = seg[max(0,i-8):i+16]
                callers.append((call_va, ctx))

print(f"  Callers de CGame::Process: {len(callers)}")
for cva, ctx in callers:
    print(f"    caller @ 0x{cva:08X}  ctx: {hexb(ctx)}")
    # Show surrounding bytes at the caller function's area
    # Find function prologue by walking back up to 200 bytes
    off = va_to_off(cva)
    if off:
        for back in range(1, 200):
            test = off - back
            if test < 0: break
            b0 = data[test]
            # common prologues: 55 (push ebp), 56 (push esi), 83 EC (sub esp)
            # Check 2-byte prologues too
            if b0 in (0x55, 0x56, 0x57) and (back > 4):
                b_1 = data[test-1] if test > 0 else 0
                # Previous byte should be CC (int3), 90 (nop), or C3/C2 (ret)
                if b_1 in (0xCC, 0x90, 0xC3, 0xC2, 0x00):
                    fn_va = cva - back
                    print(f"      ↳ possible fn start: 0x{fn_va:08X}")
                    break
            if data[test] == 0x83 and test+1 < len(data) and data[test+1] == 0xEC:
                b_1 = data[test-1] if test > 0 else 0
                if b_1 in (0xCC, 0x90, 0xC3, 0xC2, 0x00):
                    fn_va = cva - back
                    print(f"      ↳ possible fn start (sub esp): 0x{fn_va:08X}")
                    break

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("2. SetHelpMessage – analizar 0x58A330 vs 0x588BE0")
print("=" * 70)
# 0x58A330 gives: A1 CC A7 96 00 83 EC 34 83 F8 01 – looks like a real function
# (A1 = mov eax,[addr]; 83 EC 34 = sub esp,52; 83 F8 01 = cmp eax,1)
bs_a330 = read_va(0x58A330, 48)
bs_8be0 = read_va(0x588BE0, 48)
print(f"  0x58A330 (plugin-sdk canonical):")
print(f"    {hexb(bs_a330, 16)}")
print(f"    {hexb(bs_a330[16:32]) if bs_a330 else 'N/A'}")
print(f"    First byte 0x{bs_a330[0]:02X}: {'looks like MOV EAX,[mem]' if bs_a330[0]==0xA1 else 'other'}")
print(f"  0x588BE0 (mod log value):")
print(f"    {hexb(bs_8be0, 16)}")
print(f"    First byte 0x{bs_8be0[0]:02X}: 0xA0 = MOV AL,[mem]  (reading a byte flag, not a function prologue)")

# Search for callers of 0x58A330
print(f"\n  Callers de 0x58A330:")
for name, vaddr, vsize, raw_off, raw_size in sections:
    seg = data[raw_off:raw_off + raw_size]
    for i in range(len(seg) - 5):
        if seg[i] == 0xE8:
            rel = struct.unpack_from("<i", seg, i+1)[0]
            call_va = BASE + vaddr + i
            tgt_va  = call_va + 5 + rel
            if tgt_va == 0x58A330:
                print(f"    0x{call_va:08X}")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("3. Analyzing 0x439390 (declared bad VehicleCheat)")
print("=" * 70)
bs = read_va(0x439390, 64)
print(f"  Bytes at 0x439390:")
print(f"    {hexb(bs, 16)}")
print(f"    {hexb(bs[16:32])}")
print(f"    {hexb(bs[32:48])}")
# This is in the middle of code – walk back to find function start
off = va_to_off(0x439390)
print(f"  Walking back from file offset 0x{off:X} to find function prologue...")
fn_start = None
for back in range(1, 500):
    test = off - back
    if test < 0: break
    b0, b1 = data[test], data[test+1] if test+1 < len(data) else 0
    bp = data[test-1] if test > 0 else 0
    if b0 in (0x55, 0x56, 0x53) and bp in (0xCC, 0x90, 0xC3):
        fn_start = 0x439390 - back
        break
    if b0 == 0x83 and b1 == 0xEC and bp in (0xCC, 0x90, 0xC3):
        fn_start = 0x439390 - back
        break
    if b0 == 0x81 and b1 == 0xEC and bp in (0xCC, 0x90, 0xC3):
        fn_start = 0x439390 - back
        break

if fn_start:
    print(f"  Function containing 0x439390 starts at: 0x{fn_start:08X}")
    bs_fn = read_va(fn_start, 32)
    print(f"    {hexb(bs_fn, 16)}")
    print(f"    {hexb(bs_fn[16:32])}")
else:
    print("  Could not find function start automatically")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("4. Búsqueda de tabla de cheats (array de dword hash + dword fnptr)")
print("=" * 70)
# GTA SA 1.0 US: cheat hashes are stored in .text as a jump table or in .data
# as an array of {hash, fnptr} pairs. The cheat processor reads input keys,
# computes a hash, and scans the table.
# Strategy: look for dense arrays of dwords where alternating values look like
# code pointers (0x004xxxxx – 0x008xxxxx) in .data or .rdata sections.
# Also, look for the cheat input processor string "GXT" or references to 
# CCheat class methods.

# Known approach: scan .data/.rdata for arrays where every other DWORD is
# a code pointer in .text range
TEXT_LOW  = BASE + 0x1000
TEXT_HIGH = BASE + 0x457000
print(f"  Scanning .data for {{hash,fnptr}} pairs (fnptr in 0x{TEXT_LOW:08X}–0x{TEXT_HIGH:08X})...")

for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if 'data' not in sec_name.lower() and 'rdata' not in sec_name.lower():
        continue
    seg = data[raw_off:raw_off + min(raw_size, vsize)]
    best_run = 0
    best_start = 0
    run = 0
    run_start = 0
    for i in range(0, len(seg) - 8, 8):  # pairs of 2 dwords
        h = struct.unpack_from("<I", seg, i)[0]
        p = struct.unpack_from("<I", seg, i+4)[0]
        if TEXT_LOW <= p <= TEXT_HIGH:
            if run == 0:
                run_start = i
            run += 1
            if run > best_run:
                best_run = run
                best_start = run_start
        else:
            run = 0
    if best_run >= 5:
        va_start = BASE + vaddr + best_start
        print(f"\n  Section {sec_name}: best run of {best_run} pairs starting at 0x{va_start:08X}")
        # Print the first 20 entries
        for j in range(min(best_run, 25)):
            off2 = raw_off + best_start + j*8
            h = struct.unpack_from("<I", data, off2)[0]
            p = struct.unpack_from("<I", data, off2+4)[0]
            print(f"    [{j:3d}] hash=0x{h:08X}  fnptr=0x{p:08X}")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("5. CCheat::VehicleCheat – search via known caller pattern")
print("=" * 70)
# In GTA SA the cheat processing function (CCheat::DoCheats or similar)
# calls each individual cheat handler. Vehicle spawn cheat spawns a specific
# vehicle. Let's search for common cheat function patterns:
# - Functions that call CCarCtrl::CreateCarForScript or similar vehicle spawn
# - Functions near the cheat hash table

# Alternative: search for push of vehicle model IDs used in cheats
# HOTRING RACER = 494, RHINO = 432, HYDRA = 520, MONSTER = 444
# CADILLAC (Yosemite) used in BETTERTHANWALKING = 468
# RACECAR cheat uses Hotring Racer model 494 = 0x1EE
VEHICLE_MODELS = {
    "Rhino(432/0x1B0)":     struct.pack("<I", 432),
    "Hotring(494/0x1EE)":   struct.pack("<I", 494),
    "Hydra(520/0x208)":     struct.pack("<I", 520),
    "Monster(444/0x1BC)":   struct.pack("<I", 444),
    "BmxBike(481/0x1E1)":   struct.pack("<I", 481),
    "Jetpack(PICKUP=370/0x172)": struct.pack("<I", 370),
}
print("  Searching for vehicle model IDs pushed onto stack in .text:")
found_models = {}
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    for mname, mb in VEHICLE_MODELS.items():
        pos = 0
        while True:
            idx = seg.find(mb, pos)
            if idx == -1: break
            # Check if it's a PUSH imm32 (68) or MOV instruction
            b_before = seg[idx-1] if idx > 0 else 0
            va = BASE + vaddr + idx - 1
            if b_before == 0x68:  # PUSH imm32
                ctx = seg[idx-5:idx+8]
                key = f"{mname}"
                if key not in found_models:
                    found_models[key] = []
                found_models[key].append(f"0x{va:08X}")
            pos = idx + 1

for mname, locs in found_models.items():
    print(f"  {mname}:")
    for loc in locs[:5]:
        print(f"    PUSH at {loc}")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("6. CWorld::Players structure – player ped offsets")
print("=" * 70)
# CWorld::Players @ 0xB74490 is well-confirmed (376 xrefs found).
# The player ped struct (CPlayerPed inherits from CPed) has:
#   offset 0x540 = fHealth (float)   – plugin-sdk for 1.0 US
#   offset 0x544 = fMaxHealth (float)
#   offset 0x548 = fArmor (float)   – actually 0x548 in some versions
# Let's verify by finding code that reads/writes these at known xref sites

# Look at a known usage of 0xB74490 near health code
# From xrefs found: 0x004205AE, 0x00439B70
# Let's disassemble area around 0x00439B70 (looks promising – in cheat area ~0x439xxx)
print(f"\n  Analyzing code near 0x00439B70 (accesses CWorld::Players):")
bs = read_va(0x439B70 - 16, 80)
if bs:
    for i in range(0, 48, 1):
        b = bs[i]
        va = 0x439B70 - 16 + i
        if b == 0xA1:  # MOV EAX, [mem32]
            addr = struct.unpack_from("<I", bs, i+1)[0]
            print(f"    0x{va:08X}  A1  MOV EAX, [0x{addr:08X}]")
        elif b == 0x8B:
            if i+1 < len(bs):
                modrm = bs[i+1]
                print(f"    0x{va:08X}  8B {modrm:02X}  MOV r, r/m")
        elif b == 0xD9:  # FLD float
            if i+1 < len(bs):
                print(f"    0x{va:08X}  D9 {bs[i+1]:02X}  FLD/FSTP ...")
        elif b == 0xD8:
            print(f"    0x{va:08X}  D8 ...  FP op")

# More targeted: search for float stores to [reg + 0x540] pattern
# 89 86 40 05 00 00 (MOV [ESI+0x540], EAX) or D9 9E 40 05 00 00 (FSTP [ESI+0x540])
print(f"\n  Searching for [reg+0x540] (health offset) accesses:")
PAT_HEALTH = bytes([0x40, 0x05, 0x00, 0x00])  # offset 0x540 as little-endian
hits_health = []
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_HEALTH, pos)
        if idx == -1: break
        va = BASE + vaddr + idx - 2
        ctx = seg[max(0,idx-4):idx+8]
        hits_health.append((va, ctx))
        pos = idx + 1

print(f"  Hits for offset 0x540: {len(hits_health)}")
for va, ctx in hits_health[:10]:
    print(f"    0x{va:08X}: {hexb(ctx)}")

# offset 0x548 for armor
PAT_ARMOR = bytes([0x48, 0x05, 0x00, 0x00])
hits_armor = []
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_ARMOR, pos)
        if idx == -1: break
        va = BASE + vaddr + idx - 2
        ctx = seg[max(0,idx-4):idx+8]
        hits_armor.append((va, ctx))
        pos = idx + 1
print(f"  Hits for offset 0x548: {len(hits_armor)}")
for va, ctx in hits_armor[:10]:
    print(f"    0x{va:08X}: {hexb(ctx)}")

print("\n" + "=" * 70)
print("Análisis 2 completado.")
print("=" * 70)
