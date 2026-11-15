#!/usr/bin/env python3
"""
analyze8.py – Confirmar definitivamente el offset del ped dentro de CPlayerInfo.

Evidencia hasta ahora:
  0x439B70: MOV EBP, [0xB74490]; MOV EBX, [EBP+0x08]
  → *(0xB74490) es un puntero a algo, y [eso+0x08] es otro puntero.

Estrategia: leer la función 0x439B20 completa con decodificación de cada instrucción
para ver qué hace con EBP y EBX.
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

def read_va(va, n=256):
    off = va_to_off(va)
    return data[off:off+n] if off is not None else None

def hexb(bs, n=None):
    if bs is None: return "N/A"
    bs = bs[:n] if n else bs
    return ' '.join(f'{b:02X}' for b in bs)

PLAYERS = 0xB74490
REGS = ['EAX','ECX','EDX','EBX','ESP','EBP','ESI','EDI']

def decode_modrm_access(seg, i, instr_name):
    """Decode a [reg+disp32] access from ModRM at seg[i]."""
    modrm = seg[i]
    mod = (modrm >> 6) & 3
    reg = (modrm >> 3) & 7
    rm  = modrm & 7
    if mod == 2:  # [rm + disp32]
        disp = struct.unpack_from("<I", seg, i+1)[0]
        return f"{instr_name} {REGS[reg]}, [{REGS[rm]}+0x{disp:X}]", 5
    elif mod == 1:  # [rm + disp8]
        disp = seg[i+1]
        return f"{instr_name} {REGS[reg]}, [{REGS[rm]}+0x{disp:X}]", 2
    elif mod == 0:
        if rm == 5:  # disp32 direct
            addr = struct.unpack_from("<I", seg, i+1)[0]
            return f"{instr_name} {REGS[reg]}, [0x{addr:08X}]", 5
        else:
            return f"{instr_name} {REGS[reg]}, [{REGS[rm]}]", 1
    return f"? {modrm:02X}", 1

# ── Leer función 0x439B20 con contexto expandido ────────────────────────────
print("=" * 70)
print("Función 0x439B20 – decodificación completa (256 bytes)")
print("=" * 70)

fn_va = 0x439B20
fn_bs = read_va(fn_va, 256)
if fn_bs:
    i = 0
    while i < len(fn_bs) - 5:
        va = fn_va + i
        b = fn_bs[i]
        
        if b == 0xC3:
            print(f"  0x{va:08X}  C3                RET")
            break
        elif b == 0xE8:
            rel = struct.unpack_from("<i", fn_bs, i+1)[0]
            tgt = va + 5 + rel
            print(f"  0x{va:08X}  E8 ..             CALL 0x{tgt:08X}")
            i += 5
        elif b == 0x8B:
            if i+5 < len(fn_bs):
                modrm = fn_bs[i+1]
                mod = (modrm >> 6) & 3
                rm  = modrm & 7
                reg = (modrm >> 3) & 7
                if mod == 0 and rm == 5:  # direct mem
                    addr = struct.unpack_from("<I", fn_bs, i+2)[0]
                    print(f"  0x{va:08X}  8B {modrm:02X} ..        MOV {REGS[reg]}, [0x{addr:08X}]", end="")
                    if addr == PLAYERS:
                        print(f"  ← CWorld::Players", end="")
                    print()
                    i += 6
                elif mod == 1:  # [rm+disp8]
                    disp = fn_bs[i+2]
                    print(f"  0x{va:08X}  8B {modrm:02X} {disp:02X}        MOV {REGS[reg]}, [{REGS[rm]}+0x{disp:02X}]")
                    i += 3
                elif mod == 2:  # [rm+disp32]
                    disp = struct.unpack_from("<I", fn_bs, i+2)[0]
                    print(f"  0x{va:08X}  8B {modrm:02X} ..        MOV {REGS[reg]}, [{REGS[rm]}+0x{disp:08X}]")
                    i += 6
                elif mod == 3:  # reg,reg
                    print(f"  0x{va:08X}  8B {modrm:02X}           MOV {REGS[reg]}, {REGS[rm]}")
                    i += 2
                elif mod == 0:  # [rm]
                    print(f"  0x{va:08X}  8B {modrm:02X}           MOV {REGS[reg]}, [{REGS[rm]}]")
                    i += 2
                else:
                    i += 1
            else:
                i += 1
        elif b == 0x89:
            if i+5 < len(fn_bs):
                modrm = fn_bs[i+1]
                mod = (modrm >> 6) & 3
                rm  = modrm & 7
                reg = (modrm >> 3) & 7
                if mod == 1:
                    disp = fn_bs[i+2]
                    print(f"  0x{va:08X}  89 {modrm:02X} {disp:02X}        MOV [{REGS[rm]}+0x{disp:02X}], {REGS[reg]}")
                    i += 3
                elif mod == 2:
                    disp = struct.unpack_from("<I", fn_bs, i+2)[0]
                    print(f"  0x{va:08X}  89 {modrm:02X} ..        MOV [{REGS[rm]}+0x{disp:08X}], {REGS[reg]}")
                    i += 6
                elif mod == 3:
                    print(f"  0x{va:08X}  89 {modrm:02X}           MOV {REGS[rm]}, {REGS[reg]}")
                    i += 2
                else:
                    i += 1
            else:
                i += 1
        elif b == 0xD9:
            modrm = fn_bs[i+1]
            mod = (modrm >> 6) & 3
            rm  = modrm & 7
            reg = (modrm >> 3) & 7
            if mod == 2:
                disp = struct.unpack_from("<I", fn_bs, i+2)[0]
                if reg == 0:
                    print(f"  0x{va:08X}  D9 {modrm:02X} ..        FLD [{REGS[rm]}+0x{disp:08X}]")
                elif reg == 3:
                    print(f"  0x{va:08X}  D9 {modrm:02X} ..        FSTP [{REGS[rm]}+0x{disp:08X}]")
                else:
                    print(f"  0x{va:08X}  D9 {modrm:02X} ..        FP op [{REGS[rm]}+0x{disp:08X}]")
                i += 6
            elif mod == 0 and rm == 5:
                addr = struct.unpack_from("<I", fn_bs, i+2)[0]
                print(f"  0x{va:08X}  D9 05 ..          FLD [0x{addr:08X}]")
                i += 6
            else:
                print(f"  0x{va:08X}  D9 {modrm:02X}           FP")
                i += 2
        elif b == 0x85:
            modrm = fn_bs[i+1]
            reg = (modrm >> 3) & 7
            rm  = modrm & 7
            if (modrm & 0xC0) == 0xC0:
                print(f"  0x{va:08X}  85 {modrm:02X}           TEST {REGS[rm]}, {REGS[reg]}")
            else:
                print(f"  0x{va:08X}  85 ..             TEST ...")
            i += 2
        elif b == 0x0F and fn_bs[i+1] in (0x84, 0x85, 0x8C, 0x8D, 0x8E, 0x8F):
            jcc = {0x84:'JZ',0x85:'JNZ',0x8C:'JL',0x8D:'JGE',0x8E:'JLE',0x8F:'JG'}
            rel = struct.unpack_from("<i", fn_bs, i+2)[0]
            tgt = va + 6 + rel
            print(f"  0x{va:08X}  0F {fn_bs[i+1]:02X} ..       {jcc[fn_bs[i+1]]} 0x{tgt:08X}")
            i += 6
        elif b in (0x74, 0x75, 0x7E, 0x7C, 0x7F, 0x7D):
            jcc = {0x74:'JZ',0x75:'JNZ',0x7E:'JLE',0x7C:'JL',0x7F:'JG',0x7D:'JGE'}
            rel = struct.unpack_from("<b", fn_bs, i+1)[0]
            tgt = va + 2 + rel
            print(f"  0x{va:08X}  {b:02X} {fn_bs[i+1]:02X}            {jcc.get(b,'Jcc')} 0x{tgt:08X}")
            i += 2
        elif b == 0x53: print(f"  0x{va:08X}  53                PUSH EBX"); i+=1
        elif b == 0x55: print(f"  0x{va:08X}  55                PUSH EBP"); i+=1
        elif b == 0x56: print(f"  0x{va:08X}  56                PUSH ESI"); i+=1
        elif b == 0x57: print(f"  0x{va:08X}  57                PUSH EDI"); i+=1
        elif b == 0x5B: print(f"  0x{va:08X}  5B                POP EBX"); i+=1
        elif b == 0x5D: print(f"  0x{va:08X}  5D                POP EBP"); i+=1
        elif b == 0x5E: print(f"  0x{va:08X}  5E                POP ESI"); i+=1
        elif b == 0x5F: print(f"  0x{va:08X}  5F                POP EDI"); i+=1
        elif b == 0x68:
            imm = struct.unpack_from("<I", fn_bs, i+1)[0]
            print(f"  0x{va:08X}  68 ..             PUSH 0x{imm:08X}")
            i += 5
        elif b == 0x6A:
            print(f"  0x{va:08X}  6A {fn_bs[i+1]:02X}           PUSH {fn_bs[i+1]}")
            i += 2
        elif b == 0x83:
            modrm = fn_bs[i+1]
            ops = {0xC0:'ADD',0xE8:'SUB',0xF8:'CMP',0xFE:'CMP'}
            if modrm in ops:
                print(f"  0x{va:08X}  83 {modrm:02X} {fn_bs[i+2]:02X}       {ops[modrm]} EAX, {fn_bs[i+2]}")
            elif (modrm & 0xC0) == 0xC0:
                reg_ops = ['ADD','OR','ADC','SBB','AND','SUB','XOR','CMP']
                print(f"  0x{va:08X}  83 {modrm:02X} {fn_bs[i+2]:02X}       {reg_ops[(modrm>>3)&7]} {REGS[modrm&7]}, {fn_bs[i+2]}")
            elif modrm == 0xEC:
                print(f"  0x{va:08X}  83 EC {fn_bs[i+2]:02X}       SUB ESP, {fn_bs[i+2]}")
            elif modrm == 0xC4:
                print(f"  0x{va:08X}  83 C4 {fn_bs[i+2]:02X}       ADD ESP, {fn_bs[i+2]}")
            else:
                print(f"  0x{va:08X}  83 {modrm:02X} {fn_bs[i+2]:02X}       83 op")
            i += 3
        elif b == 0xA2:
            addr = struct.unpack_from("<I", fn_bs, i+1)[0]
            print(f"  0x{va:08X}  A2 ..             MOV [0x{addr:08X}], AL")
            i += 5
        elif b == 0x84:
            modrm = fn_bs[i+1]
            print(f"  0x{va:08X}  84 {modrm:02X}           TEST byte")
            i += 2
        elif b == 0x46: print(f"  0x{va:08X}  46                INC ESI"); i+=1
        elif b == 0x4E: print(f"  0x{va:08X}  4E                DEC ESI"); i+=1
        elif b == 0x4B: print(f"  0x{va:08X}  4B                DEC EBX"); i+=1
        elif b == 0x43: print(f"  0x{va:08X}  43                INC EBX"); i+=1
        elif b == 0x33:
            modrm = fn_bs[i+1]
            if (modrm & 0xC0) == 0xC0:
                print(f"  0x{va:08X}  33 {modrm:02X}           XOR {REGS[(modrm>>3)&7]}, {REGS[modrm&7]}")
            i += 2
        elif b == 0x64:
            # FS: prefix
            print(f"  0x{va:08X}  64 ..             FS: prefix")
            i += 1
        else:
            print(f"  0x{va:08X}  {b:02X}                (opcode {b:02X})")
            i += 1

print()
print("=" * 70)
print("Análisis 8 completado.")
print("=" * 70)
