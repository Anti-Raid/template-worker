import { BS, BSReader, type SerializableBytecode } from "../common"
import { IBUILTINS } from "../std"
import { APPLY_PROC_IDX, ByteCode, Closure, ClosureTemplate, OpCode, TRY_PROC_IDX } from "./vm"

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
            case OpCode.JIF:
            case OpCode.JIT: {
                line += `${padOp(OpCode[opcode])} #${inst.inst[idx + 1]}`; 
                idx += 2;
                break;
            }

            case OpCode.PUSHCONST: {
                const constIdx = inst.inst[idx + 1];
                const valStr = inst.constants ? constToString(inst.constants[constIdx]) : `[idx ${constIdx}]`;
                line += `${padOp(OpCode[opcode])} const(${valStr})`;
                idx += 2;
                break;
            }

            case OpCode.PUSHU32: {
                line += `${padOp(OpCode[opcode])} ${inst.inst[idx + 1]}`;
                idx += 2;
                break;
            }

            case OpCode.PUSHBUILTIN: {
                const proc = inst.inst[idx + 1]
                const procStr = (proc === APPLY_PROC_IDX) ? "apply" : (proc === TRY_PROC_IDX) ? `try` : `builtin(${String(IBUILTINS[proc].name)})`
                line += `${padOp(OpCode[opcode])} ${inst.inst[idx + 1]} ${procStr}`;
                idx += 2;
                break;
            }

            case OpCode.NEGATE: 
            case OpCode.DUP: 
            case OpCode.POP: 
            case OpCode.RETURN: {
                line += `${padOp(OpCode[opcode])}`;
                idx += 1;
                break;
            }

            case OpCode.PUSHUPVAR: 
            case OpCode.SETUPVAR: {
                line += `${padOp(OpCode[opcode])} upvar(${inst.inst[idx + 1]})`;
                idx += 2;
                break;
            }

            case OpCode.PUSHLOCAL:
            case OpCode.SETLOCAL: {
                line += `${padOp(OpCode[opcode])} slot(${inst.inst[idx + 1]})`;
                idx += 2;
                break;
            }
                            
            case OpCode.PUSHGLOBAL:
            case OpCode.HASGLOBAL:
            case OpCode.SETGLOBAL: {
                line += `${padOp(OpCode[opcode])} sym(${constToString(inst.constants[inst.inst[idx + 1]])})`;
                idx += 2;
                break;
            }

            case OpCode.NEWCLOSURE: {
                ops.push(`${lineNum}: ${padOp(OpCode[opcode])} tmpl(${inst.inst[idx + 1]}), closure=${constToString(inst.constants[inst.inst[idx + 1]])}`)
                const childLines = stringifyInst(inst.constants[inst.inst[idx + 1]].code)
                childLines.forEach(l => ops.push(`\t${l}`));
                idx += 2;
                continue
            }

            case OpCode.CALL:
            case OpCode.TAILCALL: {
                const nargs = inst.inst[idx + 1];
                line += `${padOp(OpCode[opcode])} nargs=${nargs}`;
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