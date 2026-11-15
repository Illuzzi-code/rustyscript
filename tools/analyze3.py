#!/usr/bin/env python3
"""
analyze3.py – Localizar CCheat::VehicleCheat correctamente.
  - Rhino PUSH @ 0x43A4A0 y 0x43A530 (Hydra) están en zona 0x43xxxx = cheat area
  - Vamos a leer el contexto completo de esas funciones
  - También: buscar tabla de hashes de cheats en zona .data cercana a 0x43xxxx
  - Encontrar CGame::Process caller function start (0x53E92D confirmado)
  - Leer offsets del ped: 0x540 health, 0x548 armor, buscar maxhealth (0x544 o diferente)
"""

import sys
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

def find_fn_start(va, max_back=800):
    """Walk backwards from va to find function prologue."""
    off = va_to_off(va)
    if off is None: return None
    for back in range(1, max_back):
        test = off - back
        if test < 1: break
        b0 = data[test]
        bp = data[test-1]
        b1 = data[test+1] if test+1 < len(data) else 0
        # 55 push ebp, preceded by CC/90/C3/C2
        if b0 in (0x55, 0x56, 0x57, 0x53) and bp in (0xCC, 0x90, 0xC3, 0x00):
            return va - back
        if b0 == 0x83 and b1 == 0xEC and bp in (0xCC, 0x90, 0xC3, 0x00):
            return va - back
        if b0 == 0x81 and b1 == 0xEC and bp in (0xCC, 0x90, 0xC3, 0x00):
            return va - back
    return None

def find_calls_in_fn(fn_va, max_len=512):
    """Extract all CALL targets from a function."""
    bs = read_va(fn_va, max_len)
    if bs is None: return []
    calls = []
    i = 0
    while i < len(bs) - 5:
        if bs[i] == 0xC3 or bs[i] == 0xC2:
            break
        if bs[i] == 0xE8:
            rel = struct.unpack_from("<i", bs, i+1)[0]
            tgt = fn_va + i + 5 + rel
            calls.append((fn_va + i, tgt))
        i += 1
    return calls

# ═══════════════════════════════════════════════════════════════════════════════
print("=" * 70)
print("1. Rhino PUSH @ 0x43A4A0 – find function and its context")
print("=" * 70)

# PUSH 432 (Rhino model) at 0x43A4A0 (the byte before is 68 = PUSH imm32)
# So the PUSH instruction is at 0x43A4A0 - 1 = 0x43A49F  ? Let's check
bs = read_va(0x43A490, 80)
print(f"  Bytes at 0x43A490:")
for row in range(0, 64, 16):
    print(f"    {hexb(bs[row:row+16])}")

# Find function start
fn_rhino = find_fn_start(0x43A4A0, 300)
if fn_rhino:
    print(f"\n  Function start: 0x{fn_rhino:08X}")
    bs_fn = read_va(fn_rhino, 32)
    print(f"    Prologue: {hexb(bs_fn, 16)}")
    # Get callers of this function
    calls_in = []
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sec_name and 'TEXT' not in sec_name:
            continue
        seg = data[raw_off:raw_off + raw_size]
        for i in range(len(seg) - 5):
            if seg[i] == 0xE8:
                rel = struct.unpack_from("<i", seg, i+1)[0]
                call_va = BASE + vaddr + i
                tgt_va  = call_va + 5 + rel
                if tgt_va == fn_rhino:
                    calls_in.append(call_va)
    print(f"  Callers of 0x{fn_rhino:08X}: {len(calls_in)}")
    for c in calls_in[:10]:
        print(f"    0x{c:08X}")
else:
    print("  Could not find function start")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("2. Hydra PUSH @ 0x43A530 – find function")
print("=" * 70)

bs = read_va(0x43A520, 48)
print(f"  Bytes at 0x43A520: {hexb(bs, 32)}")

fn_hydra = find_fn_start(0x43A530, 200)
if fn_hydra:
    print(f"  Function start: 0x{fn_hydra:08X}")
    bs_fn = read_va(fn_hydra, 32)
    print(f"    Prologue: {hexb(bs_fn, 16)}")
else:
    print("  Could not find function start")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("3. Scan for cheat hash table near 0x439390 zone")
print("=" * 70)
# 0x439390 is inside .text at file offset 0x38790.
# The area around it must be part of a larger function (the C7 44 24 xx pattern
# suggests filling a stack array with values).
# Let's read a large block around it to understand the structure.

fn_439 = find_fn_start(0x439390, 1000)
if fn_439:
    print(f"  Function containing 0x439390 starts at: 0x{fn_439:08X}")
    fn_bs = read_va(fn_439, 128)
    for row in range(0, 96, 16):
        print(f"    0x{fn_439+row:08X}: {hexb(fn_bs[row:row+16])}")
    # Find callers
    callers_439 = []
    for sec_name, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sec_name and 'TEXT' not in sec_name:
            continue
        seg = data[raw_off:raw_off + raw_size]
        for i in range(len(seg) - 5):
            if seg[i] == 0xE8:
                rel = struct.unpack_from("<i", seg, i+1)[0]
                call_va = BASE + vaddr + i
                tgt_va  = call_va + 5 + rel
                if tgt_va == fn_439:
                    callers_439.append(call_va)
    print(f"  Callers of 0x{fn_439:08X}: {len(callers_439)}")
    for c in callers_439[:10]:
        print(f"    0x{c:08X}")
else:
    # Try reading a bigger region
    print("  Walking back further...")
    bs = read_va(0x439300, 160)
    for row in range(0, 160, 16):
        print(f"    0x{0x439300+row:08X}: {hexb(bs[row:row+16])}")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("4. CGame::Process caller @ 0x53E92D – verify and read")
print("=" * 70)
bs = read_va(0x53E920, 100)
print("  Bytes at 0x53E920 (function + call to CGame::Process @ 0x53E981):")
for row in range(0, 96, 16):
    va = 0x53E920 + row
    print(f"    0x{va:08X}: {hexb(bs[row:row+16])}")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("5. Player ped health/armor/maxhealth offset confirmation")
print("=" * 70)
# We confirmed [reg+0x540] = health, [reg+0x548] = armor
# MaxHealth in plugin-sdk is 0x544. Let's verify.
PAT_MAX = bytes([0x44, 0x05, 0x00, 0x00])  # offset 0x544
hits_max = []
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_MAX, pos)
        if idx == -1: break
        va = BASE + vaddr + idx - 2
        ctx = seg[max(0,idx-4):idx+8]
        hits_max.append((va, ctx))
        pos = idx + 1
print(f"  Hits for offset 0x544 (maxhealth): {len(hits_max)}")
for va, ctx in hits_max[:8]:
    print(f"    0x{va:08X}: {hexb(ctx)}")

# Also look at the function at 0x44 known to handle player health
# From analyze2: 0x444F0D writes to [esi+0x540] = player health
# Let's see the function containing that
fn_hlt = find_fn_start(0x444F0D, 400)
if fn_hlt:
    print(f"\n  Function writing health (0x444F0D) starts at: 0x{fn_hlt:08X}")
    bs2 = read_va(fn_hlt, 64)
    for row in range(0, 48, 16):
        print(f"    0x{fn_hlt+row:08X}: {hexb(bs2[row:row+16])}")
    # Does it also write 0x544?
    fn_bytes = read_va(fn_hlt, 256)
    if fn_bytes:
        # search for 44 05 00 00 within
        idx = fn_bytes.find(bytes([0x44, 0x05, 0x00, 0x00]))
        if idx != -1:
            print(f"  → offset 0x544 also used at 0x{fn_hlt+idx-2:08X}")
        idx = fn_bytes.find(bytes([0x48, 0x05, 0x00, 0x00]))
        if idx != -1:
            print(f"  → offset 0x548 also used at 0x{fn_hlt+idx-2:08X}")

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("6. Scan for real cheat table: search .data for array of {u32,fnptr}")
print("   where fnptr values cluster together and look like cheat functions")
print("=" * 70)
# The cheat table in GTA SA 1.0 US is in .data section.
# plugin-sdk says CCheat cheat handlers are small functions near 0x438A70+
# Let's look for a dense run of function pointers in the 0x438xxx-0x43Exxx range
# and dwords that look like random 32-bit hashes (high entropy)

CHEAT_FN_LOW  = 0x438000
CHEAT_FN_HIGH = 0x43F000

for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if 'data' not in sec_name.lower() and 'rdata' not in sec_name.lower():
        continue
    seg = data[raw_off:raw_off + min(raw_size, vsize)]
    # Scan for runs of {any_dword, fnptr_in_cheat_range} pairs
    for i in range(0, len(seg) - 8, 8):
        p = struct.unpack_from("<I", seg, i+4)[0]
        if CHEAT_FN_LOW <= p <= CHEAT_FN_HIGH:
            h = struct.unpack_from("<I", seg, i)[0]
            va = BASE + vaddr + i
            # Check for a run of at least 10 consecutive pairs
            run_len = 0
            for j in range(50):
                off2 = i + j*8
                if off2+8 > len(seg): break
                p2 = struct.unpack_from("<I", seg, off2+4)[0]
                if CHEAT_FN_LOW <= p2 <= CHEAT_FN_HIGH:
                    run_len += 1
                else:
                    break
            if run_len >= 8:
                print(f"\n  Found cheat table at 0x{va:08X} (run={run_len}) in {sec_name}:")
                for j in range(min(run_len, 30)):
                    off2 = raw_off + i + j*8
                    h2 = struct.unpack_from("<I", data, off2)[0]
                    p2 = struct.unpack_from("<I", data, off2+4)[0]
                    print(f"    [{j:2d}] hash=0x{h2:08X}  fnptr=0x{p2:08X}")
                break  # found it

# ═══════════════════════════════════════════════════════════════════════════════
print("\n" + "=" * 70)
print("7. Verificación de AddMessageJumpQ – callers y convención de llamada")
print("=" * 70)
# Multiple candidates found: 0x69F1E0 (from log), 0x69F0B0, 0x69E800, 0x69E950...
# AddMessageJumpQ signature in plugin-sdk: 
#   static void AddMessageJumpQ(char *text, unsigned short time, short flag)
# It's __cdecl, 3 args (char*, u16, s16)
# Let's check callers of 0x69F1E0 to confirm
print("  Callers de 0x69F1E0 (log value):")
for sec_name, vaddr, vsize, raw_off, raw_size in sections:
    if '.text' not in sec_name and 'TEXT' not in sec_name:
        continue
    seg = data[raw_off:raw_off + raw_size]
    for i in range(len(seg) - 5):
        if seg[i] == 0xE8:
            rel = struct.unpack_from("<i", seg, i+1)[0]
            call_va = BASE + vaddr + i
            tgt_va  = call_va + 5 + rel
            if tgt_va == 0x69F1E0:
                ctx = seg[max(0,i-16):i+8]
                print(f"    caller 0x{call_va:08X}: {hexb(ctx)}")

print("\n" + "=" * 70)
print("Análisis 3 completado.")
print("=" * 70)
