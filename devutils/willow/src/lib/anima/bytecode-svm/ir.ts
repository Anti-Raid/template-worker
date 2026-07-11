import { ConstPool } from "../common";
import { ByteCode, Closure, ClosureTemplate, OpCode, type UpVarLoc } from "./vm";

export class JumpLabel {
    public id: number;
    constructor() { this.id = Math.random(); } 
}

export type JumpCond = "True" | "False"

export type Node = {
    t: "PushValue",
    constant: any 
} | {
     t: "PushBuiltin",
    builtinIdx: number
} | {
    t: "Negate"
} | {
    t: "Dup"
} | {
    t: "Pop"
} | {
    t: "PushLocal",
    slot: number 
} | {
    t: "SetLocal",
    slot: number 
} | {
    t: "PushUpvar",
    upvarIdx: number,
} | {
    t: "SetUpvar",
    upvarIdx: number,
} | {
    t: "PushGlobal",
    sym: symbol
} | {
    t: "SetGlobal",
    sym: symbol
} | {
    t: "HasGlobal",
    sym: symbol
} | {
    t: "Label",
    label: JumpLabel
} | {
    t: "CondJump", 
    label: JumpLabel,
    cond: JumpCond
} | {
    t: "Jump",
    label: JumpLabel
} | {
    t: "Call",
    nargs: number,
} | {
    t: "TailCall",
    nargs: number,
} | {
    t: "Return",
} | {
    t: "NewClosure",
    template: ClosureTemplateIR
} 

export class IR {
    constructor() {}

    lower(nodes: Node[], numLocals: number): ByteCode {
        const cpool = new ConstPool()
        const inst: number[] = []

        const jumpIdxs: Map<number, JumpLabel> = new Map()
        const resolvedLabels: Map<JumpLabel, number> = new Map()
        for(let i = 0; i < nodes.length; i++) {
            const node = nodes[i]

            switch (node.t) {
                case "PushValue": {
                    const v = node.constant

                    if (typeof v === "number") {   
                        if (Number.isInteger(v) && v >= 0 && v <= 0xFFFFFFFF) {
                            // We can use u32 specialization here
                            inst.push(OpCode.PUSHU32, v);
                        } else if (Number.isInteger(v) && v >= -1 * 0xFFFFFFFF && v < 0) {
                            // We can use u32 specialization here but we need to negate after pushing
                            inst.push(OpCode.PUSHU32, Math.abs(v));
                            inst.push(OpCode.NEGATE)
                        } else {
                            inst.push(OpCode.PUSHCONST, cpool.push(v))
                        }
                        continue
                    } else {
                        inst.push(OpCode.PUSHCONST, cpool.push(v))
                        continue
                    }
                }
                case "PushBuiltin": {
                    inst.push(OpCode.PUSHBUILTIN, node.builtinIdx)
                    break
                }
                case "Negate": {
                    inst.push(OpCode.NEGATE)
                    break
                }
                case "Dup": {
                    inst.push(OpCode.DUP)
                    break
                }
                case "Pop": {
                    inst.push(OpCode.POP)
                    break
                }
                case "PushUpvar": {
                    inst.push(OpCode.PUSHUPVAR, node.upvarIdx)
                    break
                }
                case "SetUpvar": {
                    inst.push(OpCode.SETUPVAR, node.upvarIdx)
                    break
                }
                case "PushLocal": {
                    inst.push(OpCode.PUSHLOCAL, node.slot)
                    break
                }
                case "SetLocal": {
                    inst.push(OpCode.SETLOCAL, node.slot)
                    break
                }
                case "PushGlobal": {
                    inst.push(OpCode.PUSHGLOBAL, cpool.push(node.sym))
                    break
                }
                case "SetGlobal": {
                    inst.push(OpCode.SETGLOBAL, cpool.push(node.sym))
                    break
                }
                case "HasGlobal": {
                    inst.push(OpCode.HASGLOBAL, cpool.push(node.sym))
                    break
                }
                case "Label": {
                    resolvedLabels.set(node.label, inst.length)
                    break
                }
                case "CondJump": {
                    const jidx = inst.push(node.cond === "False" ? OpCode.JIF : OpCode.JIT, -1) - 1
                    jumpIdxs.set(jidx, node.label)
                    break
                }
                case "Jump": {
                    const jidx = inst.push(OpCode.JUMP, -1) - 1
                    jumpIdxs.set(jidx, node.label)
                    break
                }
                case "Call": {
                    inst.push(OpCode.CALL, node.nargs)
                    break
                }
                case "TailCall": {
                    inst.push(OpCode.TAILCALL, node.nargs)
                    break
                }
                case "Return": {
                    inst.push(OpCode.RETURN)
                    break
                }
                case "NewClosure": {
                    const closureBc = this.lower(node.template.code, node.template.numLocals)
                    const ct = new ClosureTemplate(node.template.params, node.template.remParams, closureBc, node.template.upvarLocs)
                    if(ct.upvarLocs.length === 0) {
                        // We can just directly push the template as a raw constant in the pool
                        const cidx = cpool.mutPush(Closure.fromTemplate(ct))
                        inst.push(OpCode.PUSHCONST, cidx)
                    } else {
                        const ctidx = cpool.mutPush(ct)
                        inst.push(OpCode.NEWCLOSURE, ctidx)
                    }
                    break
                }
                default:
                    let _: never = node;
            }
        }

        for(const [jump, label] of jumpIdxs) {
            const resolvedOffset = resolvedLabels.get(label)
            if(resolvedOffset === undefined) throw new Error(`unresolved label ${label.id}`)
            if(inst[jump] !== -1) throw new Error(`inst[jump] !== -1`)
            inst[jump] = resolvedOffset
        }

        return new ByteCode(cpool.constants, new Uint32Array(inst), numLocals)
    }
}

/** A template for a closure that can then be bound to a scope */
export class ClosureTemplateIR {
    params: symbol[]; // base (individual param binds)
    remParams: symbol | null; // where the remaining params should be bound too (if any). This implicitly makes a closure variadic as well
    code: Node[]
    numLocals: number;
    upvarLocs: UpVarLoc[] // what upvars do we need to capture

    constructor(params: symbol[], remParams: symbol | null, code: Node[], numLocals: number, upvarLocs: UpVarLoc[]) {
        this.params = params
        this.remParams = remParams
        this.code = code
        this.numLocals = numLocals
        this.upvarLocs = upvarLocs
    }
}
