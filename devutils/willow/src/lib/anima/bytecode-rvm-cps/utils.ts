import { BS, BSReader, type SerializableBytecode } from "../common"
import { IBUILTINS } from "../std"
import { APPLY_PROC_IDX, BUILTINS_START, ByteCode, Closure, ClosureTemplate, OpCode } from "./vm"

const constToString = (s: any): string => {
    if(typeof s === "symbol") {
        return `${s.description || String(s)}`
    } else if (typeof s === "string") {
        return `"${s.toString()}"`
    } else if (typeof s === "number") {
        return `${s}`
    } else if (typeof s === "boolean") {
        return `<${s}>`
    } else if (typeof s === "undefined") {
        return `#<void>`
    } else if (Array.isArray(s)) {
        const r = []
        for(const elem of s) {
            r.push(constToString(elem))
        }
        return `(${r.join(' ')})`
    } else if (s instanceof ClosureTemplate) {
        return `fn(${s.params.map(x => constToString(x)).join(', ')}${s.remParams ? ` . ${constToString(s.remParams)}` : ""})`
    } else if (s instanceof Closure) {
        return `c.fn(${s.tmpl.params.map(x => constToString(x)).join(', ')}${s.tmpl.remParams ? ` . ${constToString(s.tmpl.remParams)}` : ""})`
    } {
        return `<unknown:${s}>`
    }
}

const stringifyInst = (inst: ByteCode): string[] => {
    let ops: string[] = [];
    let idx = 0;

    const padOp = (name: string) => name.padEnd(20, ' ');

    while (idx < inst.inst.length) {
        const lineNum = idx.toString().padStart(4, '0');
        const opcode: OpCode = inst.inst[idx];
        let line = `${lineNum}: `;

        switch (opcode) {                
            case OpCode.JUMP:
                line += `${padOp("JUMP")} #${inst.inst[idx + 1]}`; 
                idx += 2;
                break;

            case OpCode.LOADCONST: {
                const dest = inst.inst[idx + 1];
                const constIdx = inst.inst[idx + 2];
                const valStr = inst.constants ? constToString(inst.constants[constIdx]) : `[idx ${constIdx}]`;
                line += `${padOp("LOADCONST")} r${dest}, const(${valStr})`;
                idx += 3;
                break;
            }

            case OpCode.LOADU32:
                line += `${padOp("LOADU32")} r${inst.inst[idx + 1]}, ${inst.inst[idx + 2]}`;
                idx += 3;
                break;

            case OpCode.NEGATE:
                line += `${padOp(OpCode[opcode])} r${inst.inst[idx + 1]}`;
                idx += 2;
                break;

            case OpCode.MOVE:
            case OpCode.BOX:
            case OpCode.UNBOX:
            case OpCode.SETBOX:
                line += `${padOp(OpCode[opcode])} dest=r${inst.inst[idx + 1]}, src=r${inst.inst[idx + 2]}`;
                idx += 3;
                break;

            case OpCode.LOADUPVAR:
                line += `${padOp("LOADUPVAR")} r${inst.inst[idx + 1]}, upvar(${inst.inst[idx + 2]}) andUnbox=${inst.inst[idx + 3]}`;
                idx += 4;
                break;
                
            case OpCode.SETUPVAR:
                line += `${padOp("SETUPVAR")} r${inst.inst[idx + 1]}, upvar(${inst.inst[idx + 2]}) andBox=${inst.inst[idx + 3]}`;
                idx += 4;
                break;

            case OpCode.LOADGLOBAL:
            case OpCode.SETGLOBAL: {
                line += `${padOp(OpCode[opcode])} r${inst.inst[idx + 1]}, global(${constToString(inst.constants[inst.inst[idx + 2]])})`;
                idx += 3;
                break;
            }
            case OpCode.HASGLOBAL: {
                line += `${padOp(OpCode[opcode])} global(${constToString(inst.constants[inst.inst[idx + 1]])})`;
                idx += 2;
                break;
            }

            case OpCode.NEWCLOSURE:
                ops.push(`${lineNum}: ${padOp("NEWCLOSURE")} r${inst.inst[idx + 1]}, tmpl(${inst.inst[idx + 2]}), closure=${constToString(inst.constants[inst.inst[idx + 2]])}`)
                const childLines = stringifyInst(inst.constants[inst.inst[idx + 2]].code)
                childLines.forEach(l => ops.push(`\t${l}`));
                idx += 3;
                continue

            case OpCode.JIF:
            case OpCode.JIT:
                line += `${padOp(OpCode[opcode])} r${inst.inst[idx + 1]}, #${inst.inst[idx + 2]}`;
                idx += 3;
                break;

            case OpCode.TAILCALL: {
                const proc = inst.inst[idx + 1];
                const startReg = inst.inst[idx + 2];
                const nargs = inst.inst[idx + 3];
                const procStr = (proc === APPLY_PROC_IDX) ? "apply" : (proc < BUILTINS_START) ? `r${proc}` : `builtin(${String(IBUILTINS[proc-BUILTINS_START].name)})`
                line += `${padOp("TAILCALL")} ${procStr}, start=r${startReg}, nargs=${nargs}`;
                idx += 4;
                break;
            }

            case OpCode.LOADBASECONT: {
                const destReg = inst.inst[idx + 1];
                line += `${padOp(OpCode[opcode])} dest=r${destReg}`;
                idx += 2;
                break;
            }
            default:
                let _: never = opcode
        }
        
        ops.push(line);
    }
    
    return ops
}

export const deepPrint = (bc: ByteCode) => {
    console.log(stringifyInst(bc).join("\n"))
    for (let i = 0; i < bc.constants.length; i++) {
        const c = bc.constants[i]
        if (c instanceof ClosureTemplate) {
            console.log(`Const #${i} (template):\n${stringifyInst(c.code).join("\n")}`)
        } else if (c instanceof Closure) {
            console.log(`Const #${i} (c.fn):\n${stringifyInst(c.tmpl.code).join("\n")}`)
        }
    }
}

export const dumpFull = (b: SerializableBytecode): Uint32Array => {
    const bs = new BS()
    bs.writeValue(b)
    return bs.finalize()
}

export const readFull = (b: Uint32Array): SerializableBytecode => {
    const bsr = new BSReader(b)
    ByteCode.register(bsr)
    ClosureTemplate.register(bsr)
    Closure.register(bsr)
    return bsr.readSerializable()
}