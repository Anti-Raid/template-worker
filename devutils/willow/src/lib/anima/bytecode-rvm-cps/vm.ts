import { BS, BSReader, ErrorObject, flattenDynamicArgs, Globals, IProcedure, type SerializableBytecode } from "../common";
import { isTruthy } from "../common";
import { Cons } from "../list";
import { APPLY_PROC, ApplyProc, BuiltinFunction, IBUILTINS, TryProc } from "../std";

// Continuation (IR only)
//
// Note: this is internal
export const OP_CONT = Symbol("<cont>");
export const OP_CONT_BASECONT = Symbol("k") 

// Ptrs
export const BUILTINS_START = 2**31
export const APPLY_PROC_IDX = 2**32 - 1

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
    TAILCALL,
    NEWCLOSURE,
    BOX,
    UNBOX,
    SETBOX,
    MOVE,
    LOADBASECONT
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

class TrySuccessProc extends IProcedure {
    constructor(
        public nextCont: any, 
        public restoredHandler?: ExceptionHandler
    ) {
        super();
    }
}

// (internal+cps-form only) base cont for cps form
class CPSBaseCont extends IProcedure {}
const CPS_BASE_CONT = new CPSBaseCont()

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

type ExceptionHandler = {
    returnCont: any; // k 
    parent: ExceptionHandler | undefined;
}

type RunningCont = { type: 'RUNNING'; frame: CallFrame; parent: Continuation | null, handler?: ExceptionHandler }
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
        let rootCont: Continuation = { type: 'RUNNING', frame: initialFrame, parent: null };
        let cont: Continuation = rootCont
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
                    case OpCode.TAILCALL: {
                        const procIdx = frame.readNext()
                        const proc = (procIdx === APPLY_PROC_IDX) ? APPLY_PROC : (procIdx < BUILTINS_START) ? regs[procIdx] : IBUILTINS[procIdx - BUILTINS_START];
                        const startReg = frame.readNext();
                        const nargs = frame.readNext();
                        cont = this.#invoke(proc, cont, frame, regs, startReg, nargs);
                        break;
                    }
                    case OpCode.LOADBASECONT: {
                        const destReg = frame.readNext()
                        regs[destReg] = CPS_BASE_CONT
                        break;
                    }
                    default:
                        let _: never = opcode;
                }
            } catch (err) {
                // We either resolve the try-call or rethrow
                if (cont.type === "RUNNING" && cont.handler !== undefined) {
                    const errObj = new ErrorObject(err);
                    const currHandler = cont.handler
                    const errorCont: Continuation = {
                        type: 'RUNNING',
                        frame: frame,
                        parent: cont.parent,
                        handler: currHandler.parent
                    };

                    cont = this.#invoke(currHandler.returnCont, errorCont, frame, [errObj], 0, 1);
                    continue;
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
        startReg: number, 
        nargs: number
    ): Continuation {
        if (proc instanceof BuiltinFunction) {
            const contArg = callerArgs[startReg];
            const retVal = proc.cb(callerArgs, startReg+1, nargs-1)
            return this.#invoke(contArg, cont, callerFrame, [retVal], 0, 1);
        } else if (proc instanceof ApplyProc) {
            const contArg = callerArgs[startReg];
            const actualProc = callerArgs[startReg+1];
            const actualArgs = flattenDynamicArgs([contArg], callerArgs, startReg, nargs, "apply")
            return this.#invoke(actualProc, cont, callerFrame, actualArgs, 0, actualArgs.length);
        } else if (proc instanceof TryProc) {
            const contArg = callerArgs[startReg];
            const actualProc = callerArgs[startReg+1];

            const newHandler: ExceptionHandler = {
                returnCont: contArg,
                parent: cont.handler
            };
            const successProc = new TrySuccessProc(contArg, cont.handler);

            const actualArgs = flattenDynamicArgs([successProc], callerArgs, startReg, nargs, "apply")
            const tryCont: Continuation = { type: 'RUNNING', frame: callerFrame, parent: cont.parent, handler: newHandler };
            try {
                return this.#invoke(actualProc, tryCont, callerFrame, actualArgs, 0, actualArgs.length);
            } catch (err) {
                // Builtins error etc.
                const errObj = new ErrorObject(err);
                return this.#invoke(contArg, cont, callerFrame, [errObj], 0, 1);
            }
        } else if (proc instanceof TrySuccessProc) {
            if (nargs !== 1) throw new Error(`try-success proc continuation expected 1 argument (the final result), but got ${nargs}`)
            const resultArg = callerArgs[startReg];
            const restoreCont: Continuation = {
                type: 'RUNNING',
                frame: callerFrame,
                parent: cont.parent,
                handler: proc.restoredHandler 
            };
            return this.#invoke(proc.nextCont, restoreCont, callerFrame, [resultArg], 0, 1);
        } else if (proc instanceof Closure) {
            const template = proc.tmpl;
            const pregs = this.#createClosureArg(proc.tmpl, nargs, callerArgs, startReg)
            const nextFrame = new CallFrame(template.code, pregs, proc.upvars, 0, callerFrame.id+1);
            return { type: 'RUNNING', frame: nextFrame, parent: cont.parent, handler: cont.handler };
        } else if (proc instanceof CPSBaseCont) {
            if (nargs !== 1) throw new Error(`Base continuation expected 1 argument (the final result), but got ${nargs}`)
            return { type: 'TERMINAL', value: callerArgs[startReg] }
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