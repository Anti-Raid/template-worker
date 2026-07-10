import { ConstPool } from "../common";
import { ByteCode, Closure, ClosureTemplate, OpCode, type UpVarLoc } from "./vm";

export class JumpLabel {
    public id: number;
    constructor() { this.id = Math.random(); } 
}

export type JumpCond = "True" | "False"

export type Node = {
    t: "LoadValue",
    destReg: number,
    constant: any // will later on become a LOADCONST or a LOADU32
} | {
    t: "Move",
    destReg: number,
    srcReg: number,
} | {
    t: "LoadUpvar",
    destReg: number,
    upvarIdx: number,
    andUnbox: boolean
} | {
    t: "SetUpvar",
    srcReg: number,
    upvarIdx: number,
    andBox: boolean
} | {
    t: "LoadGlobal",
    destReg: number,
    sym: symbol
} | {
    t: "SetGlobal",
    srcReg: number,
    sym: symbol
} | {
    t: "HasGlobal",
    sym: symbol
} | {
    t: "Label",
    label: JumpLabel
} | {
    t: "CondJump", // internally specializes into JUMPIFTRUE or JUMPIFFALSE instructions later on during emission
    reg: number,
    label: JumpLabel,
    cond: JumpCond
} | {
    t: "Jump",
    label: JumpLabel
} | {
    t: "Call",
    procReg: number,
    destReg: number, // ret value is stored in destReg
    startReg: number,
    nargs: number,
} | {
    t: "TailCall",
    procReg: number,
    // does not return to caller so no ret value needed
    startReg: number,
    nargs: number,
} | {
    t: "Return",
    reg: number
} | {
    t: "NewClosure",
    destReg: number,
    template: ClosureTemplateIR
} | {
    // A call to a builtin function
    t: "IBuiltin",
    builtinIdx: number,
    destReg: number,
    startReg: number,
    nargs: number
} | {
    // A call to a builtin function
    t: "IBuiltinTail",
    builtinIdx: number,
    startReg: number,
    nargs: number
} | {
    // reg[destReg] = [reg[srcReg]]
    t: "Box",
    destReg: number,
    srcReg: number
} | {
    // reg[destReg] = reg[srcReg][0]
    t: "Unbox",
    destReg: number,
    srcReg: number
} | {
    // reg[destReg][0] = reg[srcReg]
    t: "SetBox",
    destReg: number,
    srcReg: number
}

export class IR {
    constructor() {}

    #nodeOverwritesDestReg(node: Node) {
        if(node.t === "Move" || node.t === "LoadValue" || node.t === "Box" || node.t === "Unbox" || node.t === "Call") {
            return node
        }
        return null
    }

    #numInRange(min: number, max: number, num: number) {
        return num >= min && num <= max
    }

    #nodeReadsReg(node: Node, reg: number) {
        if (node.t === "Move") return node.srcReg === reg;
        if (node.t === "Box") return node.srcReg === reg;
        if (node.t === "Unbox") return node.srcReg === reg;
        if (node.t === "SetBox") return node.srcReg === reg;
        if (node.t === "SetUpvar") return node.srcReg === reg;
        if (node.t === "SetGlobal") return node.srcReg === reg;
        if (node.t === "Call") return (node.procReg === reg || this.#numInRange(node.startReg, node.startReg + node.nargs, reg));
        if (node.t === "Return") return node.reg === reg;
        if (node.t === "TailCall") return this.#numInRange(node.startReg, node.startReg + node.nargs, reg)
        if (node.t === "Label") return false // its just a label
        if (node.t === "IBuiltin") return this.#numInRange(node.startReg, node.startReg + node.nargs, reg)
        if (node.t === "HasGlobal" || node.t === "LoadValue" || node.t === "LoadGlobal" || node.t === "LoadUpvar") return false // this reads from either const pool or upvars, not a register
        return true; // if we dont know, just assume it reads
    }

    lower(nodes: Node[], numRegs: number): ByteCode {
        const cpool = new ConstPool()
        const inst: number[] = []

        const jumpIdxs: Map<number, JumpLabel> = new Map()
        const resolvedLabels: Map<JumpLabel, number> = new Map()
        for(let i = 0; i < nodes.length; i++) {
            const node = nodes[i]

            switch (node.t) {
                case "LoadValue": {
                    // Check 1: if this is:
                    //
                    // LOAD* rY
                    // MOVE dest=rX src=rY
                    //
                    // Then we can reduce it to
                    // LOAD* rX
                    const nextNode = nodes[i+1]
                    if(nextNode && nextNode.t === "Move" && nextNode.srcReg === node.destReg) {
                        //console.log(`Ignoring next node ${JSON.stringify(nextNode)} (${i+1}) by redirecting load of ${JSON.stringify(node)} (${i})`)
                        nodes[i+1] = { t: "LoadValue", destReg: nextNode.destReg, constant: node.constant };
                        nodes.splice(i, 1);
                        i--;
                        continue;                    
                    }

                    // Check 2: is it redundant
                    let isRedundant = null; // start with the assumption that its needed
                    for (let j = i+1; j < nodes.length; j++) {
                        const nextNode = nodes[j]
                        // Stop at Labels, Jumps and CondJumps
                        if (nextNode.t === "Label" || nextNode.t === "Jump" || nextNode.t === "CondJump") break

                        if (this.#nodeReadsReg(nextNode, node.destReg)) {
                            break
                        }

                        // Check if its redundant due to dest reg overwrite
                        const nno = this.#nodeOverwritesDestReg(nextNode)
                        if(nno) {
                            // if we have a LoadValue and then a overwriting op into the same register
                            // and the overwriting op's source reg is also not the LoadValues dest, then
                            // then LoadValue is redundant
                            //
                            // Call is special: if we see a call, then we cannot omit a LoadValue if we are loading
                            // into procReg or startReg->startReg+nargs
                            const usesSameDest = nno.destReg === node.destReg
                            const isSelfRef = (nno.t === "Move" || nno.t === "Box" || nno.t === "Unbox") && nno.srcReg === node.destReg
                            if (usesSameDest && !isSelfRef) {
                                isRedundant = j
                                break
                            }
                        }
                    }

                    if(isRedundant !== null) {
                        //console.log(`Ignoring ${JSON.stringify(node)} (${i}) bc of ${JSON.stringify(this.nodes[isRedundant])} (${isRedundant})`)
                        continue
                    }

                    const v = node.constant

                    if (typeof v === "number") {   
                        if (Number.isInteger(v) && v >= 0 && v <= 0xFFFFFFFF) {
                            // We can use u32 specialization here
                            inst.push(OpCode.LOADU32, node.destReg, v);
                        } else if (Number.isInteger(v) && v >= -1 * 0xFFFFFFFF && v < 0) {
                            // We can use u32 specialization here but we need to negate after pushing
                            inst.push(OpCode.LOADU32, node.destReg, Math.abs(v));
                            inst.push(OpCode.NEGATE, node.destReg)
                        } else {
                            inst.push(OpCode.LOADCONST, node.destReg, cpool.push(v))
                        }
                        continue
                    } else {
                        inst.push(OpCode.LOADCONST, node.destReg, cpool.push(v))
                        continue
                    }
                }
                case "LoadUpvar": {
                    inst.push(OpCode.LOADUPVAR, node.destReg, node.upvarIdx, node.andUnbox ? 1 : 0)
                    break
                }
                case "SetUpvar": {
                    inst.push(OpCode.SETUPVAR, node.srcReg, node.upvarIdx, node.andBox ? 1 : 0)
                    break
                }
                case "LoadGlobal": {
                    inst.push(OpCode.LOADGLOBAL, node.destReg, cpool.push(node.sym))
                    break
                }
                case "SetGlobal": {
                    inst.push(OpCode.SETGLOBAL, node.srcReg, cpool.push(node.sym))
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
                    const jidx = inst.push(node.cond === "False" ? OpCode.JIF : OpCode.JIT, node.reg, -1) - 1
                    jumpIdxs.set(jidx, node.label)
                    break
                }
                case "Jump": {
                    const jidx = inst.push(OpCode.JUMP, -1) - 1
                    jumpIdxs.set(jidx, node.label)
                    break
                }
                case "Call": {
                    inst.push(OpCode.CALL, node.procReg, node.destReg, node.startReg, node.nargs)
                    break
                }
                case "TailCall": {
                    inst.push(OpCode.TAILCALL, node.procReg, node.startReg, node.nargs)
                    break
                }
                case "IBuiltin": {
                    inst.push(OpCode.CALL, node.builtinIdx, node.destReg, node.startReg, node.nargs)
                    break
                }
                case "IBuiltinTail": {
                    inst.push(OpCode.TAILCALL, node.builtinIdx, node.startReg, node.nargs)
                    break
                }   
                case "Return": {
                    inst.push(OpCode.RETURN, node.reg)
                    break
                }
                case "NewClosure": {
                    const closureBc = this.lower(node.template.code, node.template.numRegs)
                    const ct = new ClosureTemplate(node.template.params, node.template.remParams, closureBc, node.template.upvarLocs)
                    if(ct.upvarLocs.length === 0) {
                        // We can just directly push the template as a raw constant in the pool
                        const cidx = cpool.mutPush(Closure.fromTemplate(ct))
                        inst.push(OpCode.LOADCONST, node.destReg, cidx)
                    } else {
                        const ctidx = cpool.mutPush(ct)
                        inst.push(OpCode.NEWCLOSURE, node.destReg, ctidx)
                    }
                    break
                }
                case "Box": {
                    inst.push(OpCode.BOX, node.destReg, node.srcReg)
                    break
                }
                case "SetBox": {
                    inst.push(OpCode.SETBOX, node.destReg, node.srcReg)
                    break
                }
                case "Unbox": {
                    inst.push(OpCode.UNBOX, node.destReg, node.srcReg)
                    break
                }
                case "Move": {
                    inst.push(OpCode.MOVE, node.destReg, node.srcReg)
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

        return new ByteCode(cpool.constants, new Uint32Array(inst), numRegs)
    }
}

/** A template for a closure that can then be bound to a scope */
export class ClosureTemplateIR {
    params: symbol[]; // base (individual param binds)
    remParams: symbol | null; // where the remaining params should be bound too (if any). This implicitly makes a closure variadic as well
    code: Node[]
    numRegs: number;
    upvarLocs: UpVarLoc[] // what upvars do we need to capture

    constructor(params: symbol[], remParams: symbol | null, code: Node[], numRegs: number, upvarLocs: UpVarLoc[]) {
        this.params = params
        this.remParams = remParams
        this.code = code
        this.numRegs = numRegs
        this.upvarLocs = upvarLocs
    }
}
