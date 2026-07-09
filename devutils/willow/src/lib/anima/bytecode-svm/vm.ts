import { ErrorObject, Globals, IProcedure, isTruthy, type BS, type BSReader, type SerializableBytecode } from "../common"
import { Cons } from "../list"
import { APPLY_PROC, ApplyProc, BuiltinFunction, IBUILTINS, TRY_PROC, TryProc } from "../std"

// Ptrs
export const APPLY_PROC_IDX = 2**32 - 1
export const TRY_PROC_IDX = 2**32 - 2

export enum OpCode {
    // Push a constant from consts to the stack
    PUSHCONST, 
    // Push an unsigned 32 bit ingeger to the stack
    PUSHU32,
    // Push a builtin function or an intrinsic to top of stack
    PUSHBUILTIN,
    // Negate whatevers at top of stack
    NEGATE,
    // Push a duplicate of the top of the stack to top of stack
    //
    // Needed so OP_AND/OP_OR can DUP, then JUMPIFFALSE, preserving top of stack after the jump
    DUP,
    // Pops out the top argument of the stack
    POP,

    // Gets/sets variables
    PUSHUPVAR, // <upvar idx>
    SETUPVAR, // <upvar idx>, value on top of stack, *always* pops stack top
    PUSHLOCAL, // <slot>
    SETLOCAL, // <slot>, value on top of stack, *always* pops stack top
    PUSHGLOBAL, // <symbol>
    HASGLOBAL, // <symbol>
    SETGLOBAL, // <symbol>, value on top of stack, *always* pops stack top

    // Jump if stack top is true, *always* pops stack top
    JIT,
    // Jump if stack top is false, *always* pops stack top
    JIF,
    // Jump unconditionally
    JUMP,
    // Call a builtin or custom procedure (CALL nargs)
    CALL, 
    // Tail call a builtin or custom procedure (TAILCALL nargs). This reuses/overwrites the existing stack frame instead of creating a new one (like call does)
    TAILCALL,
    // Return from function with top value as return value. All other values are cleared from stack       
    RETURN,
    // Creates a Closure out of a ClosureTemplate (NEWCLOSURE idx) and pushes it to top of stack
    NEWCLOSURE,
}

export class ByteCode implements SerializableBytecode {
    public bsid = "ByteCode"
    constructor(public constants: any[], public inst: Uint32Array, public numLocals: number) {}
    dump(bs: BS) {
        bs.writeU32Arr(this.inst)
        bs.writeArray(this.constants)
        bs.writeU32(this.numLocals)
    }
    static register(bsr: BSReader) {
        bsr.registerFactory("ByteCode", (bsr) => {
            const inst = bsr.readU32Arr()
            const constants = bsr.readArray()
            const numLocals = bsr.readU32()
            return new ByteCode(constants, inst, numLocals)
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

const createLocalSlots = (numLocals: number) => {
    const slots = new Array(numLocals)
    for (let i = 0; i < numLocals; i++) {
        slots[i] = {value: undefined}
    }
    return slots
}

type RunningCont = {
    type: 'RUNNING';
    parent: Continuation;
    trySpot: Continuation | null | undefined;
    frame: CallFrame;
}

type Continuation = RunningCont | {
    type: "TERMINAL"
}

class CallFrame {
    constructor(
        public code: ByteCode,
        public localSlots: {value: any}[],
        public upvars: {value: any}[],
        public stack: any[],
        public ip: number
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
        let frame: CallFrame = new CallFrame(code, createLocalSlots(code.numLocals), [], [], 0);
        try {
            return this.#execnext(frame, scope);
        } catch (err: any) {
            console.log(`${err.stack}\n\nCurrent Frame IP: ${frame.ip}`)
            throw err
        }
    }

    public evaluateClosure(code: Closure, scope: Globals, args: any[]): any {
        // Initial frame
        const baseScope = this.#createClosureArg(code.tmpl, args.length, args, 0)
        let frame: CallFrame = new CallFrame(code.tmpl.code, baseScope, code.upvars, [], 0);
        try {
            return this.#execnext(frame, scope);
        } catch (err: any) {
            console.log(`${err.stack}\n\nCurrent Frame IP: ${frame.ip}`)
            throw err
        }
    }

    #execnext(initialFrame: CallFrame, execScope: Globals) {
        let cont: Continuation = { type: 'RUNNING', frame: initialFrame, parent: { type: "TERMINAL" }, trySpot: undefined }
        while(cont.type === 'RUNNING') {
            this.steps++;
            if (this.maxSteps && this.steps > this.maxSteps) {
                throw new Error(`Script ran for more than ${this.maxSteps} instructions.`);
            }

            const frame: CallFrame = cont.frame
            const stack = frame.stack
            if (frame.ip >= frame.code.inst.length) {
                throw new Error(`internal error: ${frame.ip} >= ${frame.code.inst.length}`)
            }

            try {
                const opcode: OpCode = frame.readNext()
                switch (opcode) {
                    case OpCode.PUSHCONST: {
                        const constIdx = frame.readNext()
                        stack.push(frame.getConst(constIdx));
                        break;
                    }
                    case OpCode.PUSHU32: {
                        stack.push(frame.readNext())
                        break
                    }
                    case OpCode.PUSHBUILTIN: {
                        const proc = frame.readNext()
                        const procObj = (proc === APPLY_PROC_IDX) ? APPLY_PROC : (proc === TRY_PROC_IDX) ? TRY_PROC : IBUILTINS[proc]
                        stack.push(procObj)
                        break
                    }
                    case OpCode.NEGATE: {
                        const v = stack[stack.length-1]
                        if (typeof v !== "number") throw new Error("cannot negate non-number")
                        stack[stack.length-1] = -1*v
                        break
                    }
                    case OpCode.DUP: {
                        stack.push(stack[stack.length-1])
                        break
                    }
                    case OpCode.POP: {
                        stack.pop()
                        break
                    }
                    case OpCode.PUSHUPVAR: {
                        const slot = frame.readNext()
                        stack.push(frame.upvars[slot].value)

                        break
                    }
                    case OpCode.PUSHLOCAL: {
                        const slot = frame.readNext()
                        stack.push(frame.localSlots[slot].value)
                        break
                    }
                    case OpCode.PUSHGLOBAL: {
                        const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                        stack.push(execScope.get(varname))
                        break
                    }
                    case OpCode.HASGLOBAL: {
                        const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                        let _ = execScope.get(varname)
                        break
                    }
                    case OpCode.SETGLOBAL: {
                        const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                        const val = stack.pop()
                        execScope.set(varname, val)
                        break
                    }
                    case OpCode.SETUPVAR: {
                        const slot = frame.readNext()
                        const val = stack.pop()
                        frame.upvars[slot].value = val
                        break
                    }
                    case OpCode.SETLOCAL: {
                        const slot = frame.readNext()
                        const val = stack.pop()
                        frame.localSlots[slot].value = val
                        break
                    }
                    case OpCode.JIF: {
                        const jumpIdx = frame.readNext()
                        if (isTruthy(stack.pop())) {
                            frame.ip = jumpIdx
                        }
                        break
                    }
                    case OpCode.JIT: {
                        const jumpIdx = frame.readNext()
                        if (!isTruthy(stack.pop())) {
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
                        const template = frame.getConst(frame.readNext()) as ClosureTemplate
                        const closure = Closure.fromTemplate(template)
                        // Copy over upvalues
                        for (let i = 0; i < template.upvarLocs.length; i++) {
                            const loc = template.upvarLocs[i]
                            if (loc.local) {
                                closure.upvars[i] = frame.localSlots[loc.index];
                            } else {
                                // Grab from the current frame's upvars
                                closure.upvars[i] = frame.upvars[loc.index];
                            }
                        }
                        stack.push(closure);
                        break;
                    }
                    case OpCode.RETURN: {
                        const retVal = stack.pop()
                        if(cont.parent.type === "RUNNING") {
                            cont.parent.frame.stack.push(retVal);
                            cont = cont.parent
                            continue
                        }
                        return retVal
                    }
                    case OpCode.CALL:
                    case OpCode.TAILCALL: {
                        const isTail = opcode === OpCode.TAILCALL;
                        const nargs = frame.readNext();

                        // Extract out the proc
                        const proc = stack[stack.length - 1 - nargs];
                        // Dispatch
                        cont = this.#invoke(proc, cont, frame, stack, stack.length-nargs, nargs, isTail)
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
                    const target: Continuation = cont.trySpot
                    target.frame.stack.push(retVal)
                    cont = target
                    continue
                }
                throw err
            }
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

        const callScope = createLocalSlots(template.code.numLocals);

        // required
        for (let i = 0; i < arity; i++) {
            callScope[i].value = args[startOffset+i]
        }

        // variadic
        if (template.remParams !== null) {
            const restArgs = new Array(nargs-arity);
            for (let i = 0; i < restArgs.length; i++) {
                restArgs[i] = args[startOffset + arity + i];
            }
            callScope[arity].value = restArgs
        }

        return callScope
    }

    #invoke(
        proc: any, 
        cont: RunningCont,
        callerFrame: CallFrame, 
        callerArgs: any[], // either the stack or the virtual 'stack'/caller args starting at startIdx and ending at startIdx+nargs
        startIdx: number, 
        nargs: number,
        isTail: boolean
    ): Continuation {
        if (cont.type !== "RUNNING") throw new Error("internal error: cannot invoke function using non-running cont")
        if (proc instanceof BuiltinFunction) {
            const res = proc.cb(callerArgs, startIdx, nargs)
            callerFrame.stack.push(res)
            return isTail ? cont.parent : cont
        } else if (proc instanceof ApplyProc) {
            const actualProc = callerArgs[startIdx];

            // create a virtual callerArgs to hold the arguments and copy args to it
            const actualArgs = [];
            for (let i = 1; i < nargs - 1; i++) {
                actualArgs.push(callerArgs[startIdx + i]);
            }
            const finalArg = callerArgs[startIdx + nargs - 1]
            if (Array.isArray(finalArg) || finalArg instanceof Cons) {
                actualArgs.push(...finalArg);
            } else if (finalArg === null) {
                // Empty list
            } else {
                throw new Error(`apply: last argument must be a list but got ${String(finalArg)}`);
            }
            return this.#invoke(actualProc, cont, callerFrame, actualArgs, 0, actualArgs.length, isTail)
        } else if (proc instanceof TryProc) {
            const actualProc = callerArgs[startIdx];

            // create a virtual set callerArgs to hold the arguments and copy args to it
            const actualArgs = [];
            for (let i = 1; i < nargs - 1; i++) {
                actualArgs.push(callerArgs[startIdx + i]);
            }
            const finalArg = callerArgs[startIdx + nargs - 1]
            if (Array.isArray(finalArg) || finalArg instanceof Cons) {
                actualArgs.push(...finalArg);
            } else if (finalArg === null) {
                // Empty list
            } else {
                throw new Error(`try: last argument must be a list but got ${String(finalArg)}`);
            }

            const trapCont: Continuation = isTail ? cont.parent : {
                type: 'RUNNING',
                frame: callerFrame,
                parent: cont.parent,
                trySpot: cont.trySpot
            };

            try {
                const resultingCont = this.#invoke(actualProc, cont, callerFrame, actualArgs, 0, actualArgs.length, isTail);
                
                if (resultingCont.type === "RUNNING" && resultingCont !== cont && resultingCont !== cont.parent) {
                    return {
                        ...resultingCont,
                        trySpot: trapCont
                    };
                }                
                return resultingCont;

            } catch (err) {
                // Builtins error etc.                
                if (trapCont.type !== "RUNNING") {
                    throw err
                }
                const errObj = new ErrorObject(err);
                trapCont.frame.stack.push(errObj)                
                return trapCont;
            }
        } else if (proc instanceof Closure) { 
            const template = proc.tmpl;
            const clocals = this.#createClosureArg(proc.tmpl, nargs, callerArgs, startIdx)
            const parentCont = isTail ? cont.parent : {
                type: 'RUNNING',
                frame: callerFrame,
                parent: cont.parent,
                trySpot: cont.trySpot // Preserve outer try context
            } as Continuation;
            const nextFrame = new CallFrame(template.code, clocals, proc.upvars, [], 0);
            return { type: 'RUNNING', frame: nextFrame, parent: parentCont, trySpot: cont.trySpot };
        } else {
            throw new Error(`Attempted to call a non-procedure: ${String(proc)}`);
        }
    }
}