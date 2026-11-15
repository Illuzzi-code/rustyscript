#!/usr/bin/env python3
"""
verify_all.py – Verificación exhaustiva de TODAS las direcciones del catálogo.
Para cada una: firma de bytes, callers, xrefs, coherencia estructural.
"""
import struct, sys
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

def read_va(va, n=16):
    off = va_to_off(va)
    return data[off:off+n] if off is not None else None

def hexb(bs, n=None):
    if bs is None: return "N/A"
    bs = bs[:n] if n else bs
    return ' '.join(f'{b:02X}' for b in bs)

def find_callers(target_va, limit=30):
    result = []
    for sn, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sn and 'TEXT' not in sn: continue
        seg = data[raw_off:raw_off+raw_size]
        for i in range(len(seg)-5):
            if seg[i] == 0xE8:
                rel = struct.unpack_from("<i", seg, i+1)[0]
                if BASE+vaddr+i+5+rel == target_va:
                    result.append(BASE+vaddr+i)
                    if len(result) >= limit: return result
    return result

def xrefs_to_dword(va, limit=30):
    pat = struct.pack("<I", va)
    result = []
    for sn, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sn and 'TEXT' not in sn: continue
        seg = data[raw_off:raw_off+raw_size]
        pos = 0
        while True:
            idx = seg.find(pat, pos)
            if idx == -1: break
            result.append(BASE+vaddr+idx)
            if len(result) >= limit: return result
            pos = idx+1
    return result

def is_fn_prologue(bs):
    if bs is None: return False, "no data"
    b = bs[0]
    # 55 = push ebp, 56=push esi, 53=push ebx, 57=push edi
    if b == 0x55: return True, "push ebp"
    if b in (0x56,0x53,0x57): return True, f"push reg ({b:02X})"
    # 83 EC xx = sub esp, N
    if b == 0x83 and len(bs)>1 and bs[1]==0xEC: return True, f"sub esp,{bs[2]:02X}h"
    # 81 EC xx xx xx xx = sub esp, big
    if b == 0x81 and len(bs)>1 and bs[1]==0xEC: return True, "sub esp (big)"
    # 64 A1 = FS: MOV EAX,[0] (SEH frame setup)
    if b == 0x64 and len(bs)>1 and bs[1]==0xA1: return True, "SEH frame (64 A1)"
    # A1 = MOV EAX,[mem] — common for functions reading a global first
    if b == 0xA1: return True, "MOV EAX,[mem]"
    return False, f"first byte={b:02X}"

def check_ret(va, search_range=512):
    """Find first RET within search_range bytes of va."""
    bs = read_va(va, search_range)
    if bs is None: return None
    for i, b in enumerate(bs):
        if b == 0xC3: return va+i
        if b == 0xC2: return va+i
    return None

PASS = "✅"
WARN = "⚠️ "
FAIL = "❌"

results = {}

def verify_fn(name, va, expected_first8, min_callers=0):
    bs = read_va(va, 16)
    is_fn, reason = is_fn_prologue(bs)
    callers = find_callers(va)
    ret_va  = check_ret(va)
    sig_ok  = bs is not None and list(bs[:len(expected_first8)]) == list(expected_first8)
    status  = PASS if (sig_ok and is_fn and len(callers) >= min_callers) else (WARN if sig_ok else FAIL)
    results[name] = status
    print(f"{status} {name} @ 0x{va:08X}")
    print(f"     bytes  : {hexb(bs,16)}")
    print(f"     prologue: {reason}  sig_ok={sig_ok}")
    print(f"     callers: {len(callers)}  {'(sample: '+hex(callers[0])+')' if callers else ''}")
    if ret_va: print(f"     first RET @ 0x{ret_va:08X}  (offset +{ret_va-va})")
    print()
    return status

def verify_global(name, va, expected_xrefs_min=1):
    xrefs = xrefs_to_dword(va)
    # Also check that va is in a data/bss range (above 0xB00000 for GTA SA globals)
    in_data = va > 0xB00000
    status = PASS if (len(xrefs) >= expected_xrefs_min and in_data) else WARN
    results[name] = status
    print(f"{status} {name} @ 0x{va:08X}  xrefs={len(xrefs)}  in_data_range={in_data}")
    return status

def verify_offset(name, offset, pattern_bytes, min_hits=5):
    """Verify ped/struct offset by counting [reg+offset] accesses in .text"""
    hits = 0
    for sn, vaddr, vsize, raw_off, raw_size in sections:
        if '.text' not in sn and 'TEXT' not in sn: continue
        seg = data[raw_off:raw_off+raw_size]
        pos = 0
        while True:
            idx = seg.find(pattern_bytes, pos)
            if idx == -1: break
            hits += 1
            pos = idx+1
    status = PASS if hits >= min_hits else WARN
    results[name] = status
    print(f"{status} {name} offset=+0x{offset:X}  hits={hits}  min={min_hits}")
    return status

print("=" * 68)
print("VERIFICACIÓN EXHAUSTIVA — RustyScript natives")
print("GTA SA 1.0 US  SHA-256: F01A00CE...343AC")
print("=" * 68)
print()

# ── Funciones ────────────────────────────────────────────────────────────────
print("── FUNCIONES ──────────────────────────────────────────────────────")
verify_fn("CGame::Process",    0x53BEE0,
          [0x83,0xEC,0x0C,0x53,0x56,0x57,0xE8,0xE5], min_callers=1)
verify_fn("AddMessageJumpQ",   0x69F1E0,
          [0x81,0xEC,0x20,0x03,0x00,0x00,0x56,0x8B], min_callers=1)
verify_fn("SetHelpMessage",    0x588BE0,
          [0xA0,0xC0,0xAE,0xBA,0x00,0x84,0xC0,0x0F], min_callers=1)

# Cheat stubs: pattern 68 <model u32> E8 <rel32> 59 C3
def verify_stub(name, va, expected_model):
    bs = read_va(va, 12)
    if bs is None:
        print(f"{FAIL} {name} @ 0x{va:08X}: no data")
        results[name] = FAIL; return
    ok_push = bs[0]==0x68
    model   = struct.unpack_from("<I",bs,1)[0] if ok_push else 0
    ok_call = bs[5]==0xE8
    ok_pop  = bs[10]==0x59
    ok_ret  = bs[11]==0xC3
    # decode call target
    call_tgt = 0
    if ok_call:
        rel = struct.unpack_from("<i",bs,6)[0]
        call_tgt = va+5+5+rel
    all_ok = ok_push and ok_call and ok_pop and ok_ret and model==expected_model
    status = PASS if all_ok else FAIL
    results[name] = status
    print(f"{status} {name} @ 0x{va:08X}")
    print(f"     bytes: {hexb(bs,12)}")
    print(f"     PUSH model={model} (expected {expected_model})  CALL→0x{call_tgt:08X}  POP ECX  RET")
    print()

print("── CHEAT STUBS ────────────────────────────────────────────────────")
verify_stub("spawn_rhino",   0x43A4A0, 432)
verify_stub("spawn_hotring", 0x43A4D0, 502)
verify_stub("spawn_hydra",   0x43A530, 520)
verify_stub("spawn_hunter",  0x43A550, 425)
verify_stub("spawn_monster", 0x43A4F0, 442)
verify_stub("spawn_stretch", 0x43A500, 409)
verify_stub("spawn_caddy",   0x43A520, 457)

# ── Globales ─────────────────────────────────────────────────────────────────
print("── GLOBALES ───────────────────────────────────────────────────────")
verify_global("CWorld::Players",        0xB74490, expected_xrefs_min=10)
verify_global("CClock::hours",          0xB70153, expected_xrefs_min=3)
verify_global("CClock::minutes",        0xB70152, expected_xrefs_min=3)
verify_global("CWeather::OldWeather",   0xC81320, expected_xrefs_min=3)
verify_global("CWeather::NewWeather",   0xC81324, expected_xrefs_min=3)

# ── Offsets del ped ───────────────────────────────────────────────────────────
print("── OFFSETS DEL PED ────────────────────────────────────────────────")
# [reg+0x540]: mod=10, disp32=0x00000540 → bytes 40 05 00 00
verify_offset("ped.health   +0x540", 0x540, bytes([0x40,0x05,0x00,0x00]), min_hits=10)
verify_offset("ped.maxhealth+0x544", 0x544, bytes([0x44,0x05,0x00,0x00]), min_hits=5)
verify_offset("ped.armor    +0x548", 0x548, bytes([0x48,0x05,0x00,0x00]), min_hits=5)
# Position: [reg+0x44] with mod=01 (1-byte disp) → D9 40 44 or D9 46 44 etc.
# Count all FLD/MOV [reg+0x44] where 0x44 is the disp8
hits_x = sum(
    1 for sn,va,vs,ro,rs in sections if '.text' in sn or 'TEXT' in sn
    for seg in [data[ro:ro+rs]]
    for i in range(len(seg)-2)
    if seg[i] in (0xD9,0x8B,0x89) and (seg[i+1] & 0xC7)==0x40 and seg[i+2]==0x44
)
print(f"{'✅' if hits_x>5 else '⚠️'} ped.x +0x44  FLD/MOV [reg+0x44] hits={hits_x}")
print()

# ── Verificación estructural: cadena de punteros al ped ──────────────────────
print("── CADENA DE PUNTEROS ─────────────────────────────────────────────")
# Confirm: MOV reg,[0xB74490] followed within 20 bytes by MOV reg2,[reg+0x00]
# (double deref to get ped)
PAT_LOAD = bytes([0x8B,0x2D,0x90,0x44,0xB7,0x00])  # MOV EBP,[0xB74490]
count_double = 0
for sn,vaddr,vsize,raw_off,raw_size in sections:
    if '.text' not in sn and 'TEXT' not in sn: continue
    seg = data[raw_off:raw_off+raw_size]
    pos = 0
    while True:
        idx = seg.find(PAT_LOAD, pos)
        if idx == -1: break
        # Look for MOV reg,[EBP+0x00] within next 30 bytes
        window = seg[idx+6:idx+36]
        # [EBP+0x00]: MOV reg,[EBP+0] = 8B xx 00 where xx has mod=01, rm=101(EBP)
        for j in range(len(window)-2):
            if window[j]==0x8B and (window[j+1]&0xC7)==0x45 and window[j+2]==0x00:
                count_double += 1
                break
        pos = idx+1
print(f"{'✅' if count_double>0 else '⚠️'} Double-deref pattern (MOV EBP,[Players]; MOV reg,[EBP+0]) "
      f"found {count_double} times")

# confirm EBP+0x00 access specifically (from 0x439BAA: MOV EDI,[EBP+0x00])
bs_baa = read_va(0x439BAA, 4)
expected_baa = bytes([0x8B,0x7D,0x00])  # MOV EDI,[EBP+0x00]
ok_baa = bs_baa is not None and bs_baa[:3] == expected_baa
print(f"{'✅' if ok_baa else '❌'} 0x439BAA: {hexb(bs_baa,4)} "
      f"{'== MOV EDI,[EBP+0x00] ✓' if ok_baa else '≠ expected 8B 7D 00'}")
print()

# ── Verificación AddMessageJumpQ convención de llamada ───────────────────────
print("── CONVENCIÓN DE LLAMADA AddMessageJumpQ ──────────────────────────")
# Caller 0x4681BE: check ADD ESP,10h after the CALL (= 4 args cdecl)
bs_caller = read_va(0x4681BE, 10)
print(f"  caller @ 0x4681BE: {hexb(bs_caller,10)}")
# E8 1D 70 23 00 83 C4 10 → CALL; ADD ESP,10h (=16 bytes = 4 args)
has_cleanup = bs_caller is not None and bs_caller[5]==0x83 and bs_caller[6]==0xC4 and bs_caller[7]==0x10
print(f"  {'✅' if has_cleanup else '❌'} ADD ESP,10h after CALL → {'4 args cdecl confirmed' if has_cleanup else 'NOT FOUND'}")
print()

# ── SetHelpMessage — verify single caller ────────────────────────────────────
print("── SetHelpMessage caller ──────────────────────────────────────────")
callers_help = find_callers(0x588BE0)
print(f"  Callers: {len(callers_help)}")
for c in callers_help: print(f"    0x{c:08X}: {hexb(read_va(c,10),10)}")
print()

# ── RESUMEN ───────────────────────────────────────────────────────────────────
print("=" * 68)
print("RESUMEN")
print("=" * 68)
n_pass = sum(1 for v in results.values() if v==PASS)
n_warn = sum(1 for v in results.values() if v==WARN)
n_fail = sum(1 for v in results.values() if v==FAIL)
print(f"  ✅ ALTA    : {n_pass}")
print(f"  ⚠️  MEDIA   : {n_warn}")
print(f"  ❌ BAJA/FAIL: {n_fail}")
for name, st in results.items():
    print(f"  {st} {name}")
