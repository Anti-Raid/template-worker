import { BS, BSReader, ErrorObject, flattenDynamicArgs, Globals, IProcedure, type SerializableBytecode } from "../common";
import { isTruthy } from "../common";
import { ApplyProc, BuiltinFunction, IBUILTINS, TryProc } from "../std";

export const BUILTINS_START = 2**31

export enum OpCode {
    LOADCONST,
    LOADU32,
    NEGATE,
    LOADUPVAR,
    SETUPVAR,
    LOADGLOBAL,
    SETGLOBAL,
    HASGLOBAL,
    JIF, // jump if false
    JIT, // jump if true
    JUMP, // unconditional jump
    CALL,
    TAILCALL,
    RETURN,
    NEWCLOSURE,
    BOX,
    UNBOX,
    SETBOX,
    MOVE,
}

export class ByteCode implements SerializableBytecode {
    public bsid = "ByteCode"
    constructor(public constants: any[], public inst: Uint32Array, public numReg: number) {}
    dump(bs: BS) {
        bs.writeU32Arr(this.inst)
        bs.writeArray(this.constants)
        bs.writeU32(this.numReg)
    }
    static register(bsr: BSReader) {
        bsr.registerFactory("ByteCode", (bsr) => {
            const inst = bsr.readU32Arr()
            const constants = bsr.readArray()
            const numReg = bsr.readU32()
            return new ByteCode(constants, inst, numReg)
        })
    }
}

export type UpVarLoc = { index: number, local: boolean }

/** A template for a closure that can then be bound to a scope */
export class ClosureTemplate implements SerializableBytecode {
    public bsid = "ClosureTemplate"

    params: symbol[]; // base (individual param binds)
    remParams: symbol | null; // where the remaining params should be bound too (if any). This implicitly makes a closure variadic as well
    code: ByteCode
    upvarLocs: UpVarLoc[] // what upvars do we need to capture

    constructor(params: symbol[], remParams: symbol | null, code: ByteCode, upvarLocs: UpVarLoc[]) {
        this.params = params
        this.remParams = remParams
        this.code = code
        this.upvarLocs = upvarLocs
    }

    dump(bs: BS) {
        bs.writeValue(this.params)
        bs.writeValue(this.remParams)
        bs.writeValue(this.code)
        bs.writeValue(this.upvarLocs)
    }
    static register(bsr: BSReader) {
        bsr.registerFactory("ClosureTemplate", (bsr) => {
            const params = bsr.read() as symbol[]
            const remParams = bsr.read() as symbol | null
            const code = bsr.readSerializable<ByteCode>("ByteCode")
            const upvarLocs = bsr.readArray() as UpVarLoc[]
            return new ClosureTemplate(params, remParams, code, upvarLocs)
        })
    }
}

/** An actual anima closure bound to a scope */
export class Closure extends IProcedure implements SerializableBytecode {
    public bsid = "Closure"
    constructor(public tmpl: ClosureTemplate, public upvars: any[]) {
        super()
    }

    static fromTemplate(tmpl: ClosureTemplate) {
        // Allocate enough space for the upvars from outer scopes
        const upvars = new Array(tmpl.upvarLocs.length)
        return new Closure(tmpl, upvars)
    }

    dump(bs: BS) {
        bs.writeValue(this.upvars)
        bs.writeValue(this.tmpl)
    }
    static register(bsr: BSReader) {
        bsr.registerFactory("Closure", (bsr) => {
            const upvars = bsr.readArray()
            const tmpl = bsr.readSerializable<ClosureTemplate>("ClosureTemplate")
            return new Closure(tmpl, upvars)
        })
    }
}

const createRegs = (numRegs: number) => {
    return new Array(numRegs).fill(undefined)
}

const regOut = (reg: any): any => {
    if (reg instanceof Box) return `Box<${regOut(reg.val)}>`
    if (reg instanceof Closure) return `Closure<${reg.tmpl.params.map(x => x.description).join(", ")}>`
    if (typeof reg === "symbol") return `${reg.description || '<symbol>'}`
    if (reg === undefined) return '#<void>'
    if (reg === null) return `()`
    if (typeof reg !== "object") return `${reg}`
    return `<object ${Object.keys(reg)}>`
}

// To make life debugging registers easier
class Box {
    constructor(public val: any) {}
}

type RunningCont = { type: 'RUNNING'; frame: CallFrame; parent: Continuation | null, trySpot: Continuation | null | undefined, destReg?: number }
type Continuation = RunningCont
| { type: "TERMINAL", value: any }

class CallFrame {
    constructor(
        public code: ByteCode,
        public regs: any[],
        public upvars: any[],
        public ip: number,
        public id: number,
    ) {}

    readNext() {
        if (this.ip >= this.code.inst.length) {
            throw new Error(`internal error: unexpected end of bytecode, instruction pointer out of bounds (${this.ip} >= ${this.code.inst.length}).`);
        }
        return this.code.inst[this.ip++]
    }

    getConst(idx: number) {
        return this.code.constants[idx]
    }
}

export class AnimaVM {
    constructor(public steps: number = 0, public maxSteps: number = 0) {}

    public evaluateRaw(code: ByteCode, scope: Globals): any {
        // Initial frame
        let frame: CallFrame = new CallFrame(code, createRegs(code.numReg), [], 0, 0);
        try {
            return this.#execnext(frame, scope);
        } catch (err: any) {
            console.log(`${err.stack}\n\nCurrent Frame IP: ${frame.ip}`)
            throw err
        }
    }

    public evaluateClosure(code: Closure, scope: Globals, args: any[]): any {
        // Initial frame
        const cargs = this.#createClosureArg(code.tmpl, args.length, args, 0)
        let frame: CallFrame = new CallFrame(code.tmpl.code, cargs, code.upvars, 0, 0);
        try {
            return this.#execnext(frame, scope);
        } catch (err: any) {
            console.log(`${err.stack}\n\nCurrent Frame IP: ${frame.ip}`)
            throw err
        }
    }

    #execnext(initialFrame: CallFrame, execScope: Globals) {
        let cont: Continuation = { type: 'RUNNING', frame: initialFrame, parent: null, trySpot: undefined }
        while(cont.type === 'RUNNING') {
            this.steps++;
            if (this.maxSteps && this.steps > this.maxSteps) {
                throw new Error(`Script ran for more than ${this.maxSteps} instructions.`);
            }

            const frame: CallFrame = cont.frame
            const regs = frame.regs
            if (frame.ip >= frame.code.inst.length) {
                throw new Error(`internal error: ${frame.ip} >= ${frame.code.inst.length}`)
            }

            try {
                const opcode: OpCode = frame.readNext()
                //console.log(`[Frame #${frame.id}]: ${OpCode[opcode]} ${regs.map(r => regOut(r)).join(', ')}`)
                switch (opcode) {
                    // Load
                    case OpCode.LOADCONST: {
                        const destReg = frame.readNext()
                        const constIdx = frame.readNext()
                        regs[destReg] = frame.getConst(constIdx);
                        break;
                    }
                    case OpCode.LOADU32: {
                        const destReg = frame.readNext()
                        const u32Val = frame.readNext()
                        regs[destReg] = u32Val 
                        break
                    }
                    case OpCode.NEGATE: {
                        const reg = frame.readNext()
                        if (typeof regs[reg] !== "number") throw new Error("cannot negate non-number")
                        regs[reg] = -1*regs[reg] 
                        break
                    }
                    case OpCode.LOADUPVAR: {
                        const destReg = frame.readNext()
                        const upvarIdx = frame.readNext()
                        const andUnbox = frame.readNext()
                        regs[destReg] = andUnbox ? (frame.upvars[upvarIdx] as Box).val : frame.upvars[upvarIdx]
                        break
                    }
                    case OpCode.SETUPVAR: {
                        const srcReg = frame.readNext()
                        const upvarIdx = frame.readNext()
                        const andBox = frame.readNext()
                        frame.upvars[upvarIdx] = andBox ? new Box(regs[srcReg]) : regs[srcReg]
                        break
                    }
                    case OpCode.LOADGLOBAL: {
                        const destReg = frame.readNext()
                        const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                        regs[destReg] = execScope.get(varname)
                        break
                    }
                    case OpCode.SETGLOBAL: {
                        const srcReg = frame.readNext()
                        const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                        execScope.set(varname, regs[srcReg])
                        break
                    }
                    case OpCode.HASGLOBAL: {
                        const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                        execScope.assert(varname)
                        break
                    }
                    case OpCode.JIF: {
                        const condReg = frame.readNext()
                        const jumpIdx = frame.readNext()
                        if (!isTruthy(regs[condReg])) {
                            frame.ip = jumpIdx
                        }
                        break
                    }
                    case OpCode.JIT: {
                        const condReg = frame.readNext()
                        const jumpIdx = frame.readNext()
                        if (isTruthy(regs[condReg])) {
                            frame.ip = jumpIdx
                        }
                        break
                    }
                    case OpCode.JUMP: {
                        const jumpIdx = frame.readNext()
                        frame.ip = jumpIdx
                        break
                    }
                    case OpCode.NEWCLOSURE: {
                        const destReg = frame.readNext()
                        const tidx = frame.readNext()
                        const template = frame.getConst(tidx) as ClosureTemplate
                        const closure = Closure.fromTemplate(template)
                        // Copy over upvalues
                        for (let i = 0; i < template.upvarLocs.length; i++) {
                            const loc = template.upvarLocs[i]
                            if (loc.local) {
                                closure.upvars[i] = regs[loc.index];
                            } else {
                                // Grab from the current frame's upvars
                                closure.upvars[i] = frame.upvars[loc.index];
                            }
                        }
                        regs[destReg] = closure
                        break;
                    }
                    case OpCode.BOX: {
                        const destReg = frame.readNext();
                        const srcReg = frame.readNext();
                        regs[destReg] = new Box(regs[srcReg])
                        break
                    }
                    case OpCode.UNBOX: {
                        const destReg = frame.readNext();
                        const srcReg = frame.readNext();
                        regs[destReg] = (regs[srcReg] as Box).val
                        break
                    }
                    case OpCode.SETBOX: {
                        const destReg = frame.readNext();
                        const srcReg = frame.readNext();
                        (regs[destReg] as Box).val = regs[srcReg]
                        break
                    }
                    case OpCode.MOVE: {
                        const destReg = frame.readNext();
                        const srcReg = frame.readNext();
                        regs[destReg] = regs[srcReg]
                        break
                    }
                    case OpCode.RETURN: {
                        const reg = frame.readNext()
                        const retVal = frame.regs[reg];
                        if (cont.parent === null) {
                            cont = { type: 'TERMINAL', value: retVal };
                        } else {
                            const parent: Continuation = cont.parent;
                            if (parent.type === "RUNNING" && parent.destReg !== undefined) {
                                parent.frame.regs[parent.destReg] = retVal;
                            }
                            cont = parent; // Jump back to the parent continuation
                        }
                        break;               
                    }
                    case OpCode.CALL: {
                        const procIdx = frame.readNext()
                        const proc = (procIdx < BUILTINS_START) ? regs[procIdx] : IBUILTINS[procIdx - BUILTINS_START];
                        const destReg = frame.readNext();
                        const startReg = frame.readNext();
                        const nargs = frame.readNext();
                        cont = this.#invoke(proc, cont, frame, regs, destReg, startReg, nargs)
                        break;
                    }
                    case OpCode.TAILCALL: {
                        const procIdx = frame.readNext()
                        const proc = (procIdx < BUILTINS_START) ? regs[procIdx] : IBUILTINS[procIdx - BUILTINS_START];
                        const startReg = frame.readNext();
                        const nargs = frame.readNext();
                        cont = this.#invoke(proc, cont, frame, regs, undefined, startReg, nargs);
                        break;
                    }
                    default:
                        let _: never = opcode;
                }
            } catch (err) {
                // We either resolve the try-call or rethrow
                if (cont.type === "RUNNING" && cont.trySpot !== undefined) {
                    const retVal = new ErrorObject(err)
                    if (cont.trySpot === null || cont.trySpot.type !== "RUNNING") {
                        return retVal
                    }
                    const target: RunningCont = cont.trySpot

                    if (target.destReg !== undefined) {
                        target.frame.regs[target.destReg] = retVal;
                    }

                    cont = target
                    continue
                }
                throw err
            }
        }

        return cont.value
    }

    // Note: if destReg is not set, we treat it as a tailcall
    #invoke(
        proc: any, 
        cont: RunningCont,
        callerFrame: CallFrame, 
        callerArgs: any[], 
        destReg: number | undefined, 
        startReg: number, 
        nargs: number
    ): Continuation {
        if (proc instanceof BuiltinFunction) {
            const retVal = proc.cb(callerArgs, startReg, nargs)
            if (destReg !== undefined) {
                callerFrame.regs[destReg] = retVal
            } else {
                if (cont.parent === null) return { type: 'TERMINAL', value: retVal };
                
                const parent = cont.parent;
                if (parent.type === "RUNNING" && parent.destReg !== undefined) {
                    parent.frame.regs[parent.destReg] = retVal;
                }
                return parent;
            }
            return cont // no change to continuation needed
        } else if (proc instanceof ApplyProc) {
            const actualProc = callerArgs[startReg];
            const actualArgs = flattenDynamicArgs([], callerArgs, startReg, nargs, "apply")
            return this.#invoke(actualProc, cont, callerFrame, actualArgs, destReg, 0, actualArgs.length)
        } else if (proc instanceof TryProc) {
            const actualProc = callerArgs[startReg];
            const actualArgs = flattenDynamicArgs([], callerArgs, startReg, nargs, "try")

            const trapCont: Continuation | null = (destReg === undefined) ? cont.parent : {
                type: 'RUNNING',
                frame: callerFrame,
                parent: cont.parent,
                destReg: destReg,
                trySpot: cont.trySpot
            };

            try {
                const resultingCont = this.#invoke(actualProc, cont, callerFrame, actualArgs, destReg, 0, actualArgs.length);
                
                if (resultingCont.type === "RUNNING" && resultingCont !== cont && resultingCont !== cont.parent) {
                    return {
                        ...resultingCont,
                        trySpot: trapCont
                    };
                }                
                return resultingCont;

            } catch (err) {
                // Builtins error etc.
                const errObj = new ErrorObject(err);
                
                if (trapCont === null || trapCont.type !== "RUNNING") {
                    return { type: 'TERMINAL', value: errObj };
                }
                
                if (trapCont.destReg !== undefined) {
                    trapCont.frame.regs[trapCont.destReg] = errObj;
                }
                
                return trapCont;
            }
        } else if (proc instanceof Closure) {
            const template = proc.tmpl;
            const pregs = this.#createClosureArg(proc.tmpl, nargs, callerArgs, startReg)
            const parentCont = (destReg === undefined) ? cont.parent : {
                type: 'RUNNING',
                frame: callerFrame,
                parent: cont.parent,
                destReg: destReg,
                trySpot: cont.trySpot // Preserve outer try context
            } as Continuation | null;
            const nextFrame = new CallFrame(template.code, pregs, proc.upvars, 0, callerFrame.id+1);
            return { type: 'RUNNING', frame: nextFrame, parent: parentCont, trySpot: cont.trySpot };
        } else {
            throw new Error(`Attempted to call a non-procedure: ${String(proc)}`);
        }
    }

    #createClosureArg(template: ClosureTemplate, nargs: number, args: any[], startOffset: number) {
        const arity = template.params.length; // number of required args
        if (template.remParams !== null) {
            // variadic
            if (nargs < arity) {
                throw new Error(`expected at least ${arity} args, got ${nargs}`);
            }
        } else {
            if (nargs !== arity) {
                throw new Error(`expected exactly ${arity} args, got ${nargs}`);
            }
        }

        const closureRegs = createRegs(template.code.numReg)

        // required
        for (let i = 0; i < arity; i++) {
            closureRegs[i] = args[startOffset+i]
        }

        // variadic
        if (template.remParams !== null) {
            const restArgs = new Array(nargs-arity);
            for (let i = 0; i < restArgs.length; i++) {
                restArgs[i] = args[startOffset + arity + i];
            }
            closureRegs[arity] = restArgs
        }

        return closureRegs
    }
}