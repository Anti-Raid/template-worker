import { ExposedProps, IProcedure, isDeepEqual, MissingVarError, OP_ADD, OP_CAR, OP_CDR, OP_CONS, OP_CONTAINS, OP_DIV, OP_EMPTY, OP_EQ, OP_GT, OP_GTE, OP_LAST, OP_LENGTH, OP_LIST, OP_LT, OP_LTE, OP_MEMBER, OP_MODULO, OP_MUL, OP_NOT, OP_REMAINDER, OP_SUB } from "../common";
import { isTruthy } from "../common";
import { Cons } from "../list";

export enum OpCode {
    LOADCONST, 
    LOADTRUE,
    LOADFALSE,
    LOADEMPTYLIST,
    LOADVOID,
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
    BUILTINCALL,
    RETURN,
    NEWCLOSURE,
    // may be removed later
    BOX,
    UNBOX,
    SETBOX,
    // not yet used
    MOVE,
}

export class ByteCode {
    public constants: any[]
    public inst: Uint32Array
    public numReg: number
    constructor(constants: any[], inst: Uint32Array, numReg: number) {
        this.constants = constants
        this.inst = inst
        this.numReg = numReg
    }
}

export type UpVarLoc = { index: number, local: boolean }

/** A template for a closure that can then be bound to a scope */
export class ClosureTemplate {
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
}

/** An actual anima closure bound to a scope */
export class Closure extends IProcedure {
    upvars: any[]
    constructor(public tmpl: ClosureTemplate) {
        super()
        // Allocate enough space for the upvars from outer scopes
        this.upvars = new Array(tmpl.upvarLocs.length)
    }
}

/** 
 * A builtin function. Builtin functions do not have access to their own lexical scope (at least not yet) 
*/
export class BuiltinFunction extends IProcedure {
    constructor(
        public name: symbol,
        public cb: (regs: any[], destReg: number, startReg: number, nargs: number) => void,
    ) {
        super()
    }
}

// Stores all of our builtin funcs
export const IBUILTINS: BuiltinFunction[] = [
    new BuiltinFunction(OP_ADD, (regs, destReg, startReg, nargs) => {
        let acc = 0; 
        for (let i = startReg; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`+ requires numbers, but received ${typeof val}`);
            acc += val
        }
        regs[destReg] = acc
    }),
    new BuiltinFunction(OP_SUB, (regs, destReg, startReg, nargs) => {
        if (nargs === 0) throw new Error("- requires at least 1 argument");
        
        if (nargs === 1) {
            const val = regs[startReg];
            if (typeof val !== "number") throw new Error(`- requires numbers, but received ${typeof val}`);
            regs[destReg] = -val; 
            return;
        }

        let acc = regs[startReg];
        if (typeof acc !== "number") throw new Error(`- requires numbers, but received ${typeof acc}`);
        for (let i = startReg + 1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`- requires numbers, but received ${typeof val}`);
            acc -= val
        }
        regs[destReg] = acc
    }),
    new BuiltinFunction(OP_MUL, (regs, destReg, startReg, nargs) => {
        let acc = 1; 
        for (let i = startReg; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`* requires numbers, but received ${typeof val}`);
            acc *= val
        }
        regs[destReg] = acc
    }),
    new BuiltinFunction(OP_DIV, (regs, destReg, startReg, nargs) => {
        if (nargs === 0) throw new Error("/ requires at least 1 argument");
        
        if (nargs === 1) {
            const val = regs[startReg];
            if (typeof val !== "number") throw new Error(`/ requires numbers, but received ${typeof val}`);
            if (val === 0) throw new Error("division by zero");
            regs[destReg] = 1/val; 
            return;
        }

        let acc = regs[startReg];
        if (typeof acc !== "number") throw new Error(`/ requires numbers, but received ${typeof acc}`);
        for (let i = startReg + 1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`/ requires numbers, but received ${typeof val}`);
            if (val === 0) throw new Error("division by zero");
            acc /= val
        }
        regs[destReg] = acc
    }),
    new BuiltinFunction(OP_MODULO, (regs, destReg, startReg, nargs) => {
        if(nargs !== 2) throw new Error("modulo requires 2 arguments");
        const a = regs[startReg] 
        const b = regs[startReg+1]
        if (typeof a !== "number" || typeof b !== "number") throw new Error(`modulo: requires numbers, but received ${typeof a}/${typeof b}`);
        if (b === 0) throw new Error("modulo: division by zero");
        regs[destReg] = ((a % b) + b) % b
    }),
    new BuiltinFunction(OP_REMAINDER, (regs, destReg, startReg, nargs) => {
        if(nargs !== 2) throw new Error("remainder requires 2 arguments");
        const a = regs[startReg] 
        const b = regs[startReg+1]
        if (typeof a !== "number" || typeof b !== "number") throw new Error(`remainder: requires numbers, but received ${typeof a}/${typeof b}`);
        if (b === 0) throw new Error("remainder: division by zero");
        regs[destReg] = a % b
    }),
    new BuiltinFunction(OP_LIST, (regs, destReg, startReg, nargs) => {
        const lst = new Array(nargs)
        for(let i = 0; i < nargs; i++) {
            lst[i] = regs[startReg+i]
        }
        regs[destReg] = lst
    }),
    new BuiltinFunction(OP_EQ, (regs, destReg, startReg, nargs) => {
        if (nargs === 0) throw new Error("= requires at least 1 argument");
        
        let start = regs[startReg];
        if (typeof start !== "number") throw new Error(`= requires numbers, but received ${typeof start}`);
        let res = true
        for (let i = startReg+1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`= requires numbers, but received ${typeof val}`);
            if (val !== start) {
                res = false
                break
            }
        }
        regs[destReg] = res
    }),
    new BuiltinFunction(OP_LT, (regs, destReg, startReg, nargs) => {
        if (nargs === 0) throw new Error("< requires at least 1 argument");
        
        let start = regs[startReg];
        if (typeof start !== "number") throw new Error(`< requires numbers, but received ${typeof start}`);
        let res = true
        for (let i = startReg+1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`< requires numbers, but received ${typeof val}`);
            if (val < start) {
                res = false
                break
            }
        }
        regs[destReg] = res
    }),
    new BuiltinFunction(OP_LTE, (regs, destReg, startReg, nargs) => {
        if (nargs === 0) throw new Error("<= requires at least 1 argument");
        
        let start = regs[startReg];
        if (typeof start !== "number") throw new Error(`<= requires numbers, but received ${typeof start}`);
        let res = true
        for (let i = startReg+1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`<= requires numbers, but received ${typeof val}`);
            if (val <= start) {
                res = false
                break
            }
        }
        regs[destReg] = res
    }),
    new BuiltinFunction(OP_GT, (regs, destReg, startReg, nargs) => {
        if (nargs === 0) throw new Error("> requires at least 1 argument");
        
        let start = regs[startReg];
        if (typeof start !== "number") throw new Error(`> requires numbers, but received ${typeof start}`);
        let res = true
        for (let i = startReg+1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`> requires numbers, but received ${typeof val}`);
            if (val > start) {
                res = false
                break
            }
        }
        regs[destReg] = res
    }),
    new BuiltinFunction(OP_GTE, (regs, destReg, startReg, nargs) => {
        if (nargs === 0) throw new Error(">= requires at least 1 argument");
        
        let start = regs[startReg];
        if (typeof start !== "number") throw new Error(`>= requires numbers, but received ${typeof start}`);
        let res = true
        for (let i = startReg+1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`>= requires numbers, but received ${typeof val}`);
            if (val > start) {
                res = false
                break
            }
        }
        regs[destReg] = res
    }),
    // list builtins
    new BuiltinFunction(OP_CAR, (regs, destReg, startReg, nargs) => {
        if (nargs !== 1) throw new Error("car requires 1 argument");
        const val = regs[startReg]
        if (Array.isArray(val)) {
            if (val.length < 1) throw new Error("car requires a non-empty list");
            regs[destReg] = val[0];
        } else if (val instanceof Cons) {
            regs[destReg] = val.head;
        } else if (val === null) {
            throw new Error("car requires a non-empty list");
        } else {
            throw new Error("car requires a list");
        }
    }),
    new BuiltinFunction(OP_CDR, (regs, destReg, startReg, nargs) => {
        if (nargs !== 1) throw new Error("cdr requires 1 argument");
        const val = regs[startReg]
        if (Array.isArray(val)) {
            if (val.length < 1) throw new Error("cdr requires a non-empty list");
            regs[destReg] = Cons.fromArray(val, 1)
        } else if (val instanceof Cons) { 
            regs[destReg] = val.tail
        } else if (val === null) {
            throw new Error("cdr requires a non-empty list");
        } else {
            throw new Error(`cdr requires a list but got ${val}`);
        }
    }),
    new BuiltinFunction(OP_CONS, (regs, destReg, startReg, nargs) => {
        if (nargs != 2) throw new Error("cons requires 2 arguments [cons a d]");
        regs[destReg] = Cons.pair(regs[startReg], regs[startReg+1])
    }),
    new BuiltinFunction(OP_LAST, (regs, destReg, startReg, nargs) => {
        if (nargs != 1) throw new Error("last requires 1 argument");
        const val = regs[startReg]
        if (Array.isArray(val)) {
            if (val.length < 1) throw new Error("last requires a non-empty list");
            regs[destReg] = val[val.length - 1];
        } else if (val instanceof Cons) {
            let last = val.head
            for(const v of val) {
                last = v;
            }
            regs[destReg] = last
        } else if (val === null) {
            throw new Error("last requires a non-empty list");
        } else {
            throw new Error("last requires a list");
        }
    }),
    new BuiltinFunction(OP_LENGTH, (regs, destReg, startReg, nargs) => {
        if (nargs != 1) throw new Error("length requires 1 argument");
        const val = regs[startReg]
        if (val === null) {
            regs[destReg] = 0 // empty list
            return
        }
        // TODO: Add string-length? for strings specifically like scheme does
        regs[destReg] = (Array.isArray(val) || val instanceof Cons) ? val.length : (typeof val === "string" ? val.length : 0)
    }),
    new BuiltinFunction(OP_EMPTY, (regs, destReg, startReg, nargs) => {
        if (nargs != 1) throw new Error("empty requires 1 argument");
        const val = regs[startReg]
        if (val === null) {
            regs[destReg] = true // empty list
            return
        }
        regs[destReg] = (Array.isArray(val) || val instanceof Cons) ? (val.length == 0) : (typeof val === "string" ? (val.length == 0) : false);
    }),
    new BuiltinFunction(OP_CONTAINS, (regs, destReg, startReg, nargs) => {
        if (nargs != 2) throw new Error("contains? requires 2 arguments");
        const list = regs[startReg]
        const item = regs[startReg+1]
        regs[destReg] = (Array.isArray(list) || list instanceof Cons) ? list.includes(item) : false;
    }),
    new BuiltinFunction(OP_MEMBER, (regs, destReg, startReg, nargs) => {
        if (nargs != 2) throw new Error("member? requires 2 arguments");
        let list = regs[startReg] as (any[] | Cons | null)
        const item = regs[startReg+1]
        if (Array.isArray(list)) {
            for (let i = 0; i < list.length; i++) {
                if (isDeepEqual(list[i], item)) {
                    // create a view of the array starting from i
                    regs[destReg] = Cons.fromArray(list, i)
                    return;
                }
            }
            regs[destReg] = false;
            return;
        }
        else if (list === null) {
            regs[destReg] = false // not a member if empty list
            return
        }
        else if (!(list instanceof Cons)) throw new Error("member? requires the first argument to be a list")
        list = list as Cons // cast to Cons for type safety
        let s: Cons | null = list
        while(s !== null) {
            if (isDeepEqual(s.head, item)) {
                regs[destReg] = s
                return
            }
            s = s.tail
        }
        regs[destReg] = false
    }),
    new BuiltinFunction(OP_NOT, (regs, destReg, startReg, nargs) => {
        if (nargs != 1) throw new Error("not requires 1 argument");
        regs[destReg] = !isTruthy(regs[startReg]);
    })
]

// Marker for `apply` intrinsic proc
export class ApplyProc extends IProcedure {}

/** The register block the actual anima engine has/uses */
class RegBlock {
    public regs: any[]
    constructor(numRegs: number) {
        this.regs = new Array(numRegs).fill(undefined)
    }

    /*box(dest: number, src: number) {
        this.regs[dest] = [this.regs[src]]
    }
    unbox(dest: number, src: number) {
        this.regs[dest] = this.regs[src][0]
    }
    setbox(dest: number, src: number) {
        this.regs[dest][0] = this.regs[src]
    }
    move(dest: number, src: number) {
        this.regs[dest] = this.regs[src]
    }*/
}

// To make life debugging registers easier
class Box {
    constructor(public val: any) {}
}

export class Globals {
    constructor(public data: Map<symbol, any>, public frozen: boolean = false) {}

    static newWith(fields: Record<symbol, any>, frozen: boolean = false) {
        const map = new Map()
        Object.getOwnPropertySymbols(fields).forEach((sym) => {
            map.set(sym, fields[sym])
        });
        return new Globals(map, frozen);
    }
}

class CallFrame {
    constructor(
        public code: ByteCode,
        public regs: RegBlock,
        public upvars: any[],
        public ip: number,
        public retReg: number,
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

    public evaluate(code: ByteCode, scope: Globals, props?: ExposedProps): any {
        // Initial frame
        let frames: CallFrame[] = [new CallFrame(code, new RegBlock(code.numReg), [], 0, 0)];
        try {
            return this.#evalinner(frames, scope, props);
        } catch (err: any) {
            console.log(`${err.stack}\n\nFrame #${frames.length-1}\nCurrent Frame IP: ${frames[frames.length-1].ip}`)
        }
    }

    #evalinner(frames: CallFrame[], execScope: Globals, props?: ExposedProps): any {
        while (frames.length > 0) {
            this.steps++;
            if (this.maxSteps && this.steps > this.maxSteps) {
                throw new Error(`Script ran for more than ${this.maxSteps} instructions.`);
            }

            const frame = frames[frames.length - 1];
            const regs = frame.regs.regs

            if (frame.ip >= frame.code.inst.length) {
                const lf = frames.pop();
                if (lf) {
                    frames[frames.length-1].regs.regs[lf.retReg] = false // no return so treat as false implicitly
                }
                continue;
            }

            const opcode: OpCode = frame.readNext()
            //console.log(`${OpCode[opcode]} ${regs.join(', ')}`)
            switch (opcode) {
                // Load
                case OpCode.LOADCONST: {
                    const destReg = frame.readNext()
                    const constIdx = frame.readNext()
                    regs[destReg] = frame.getConst(constIdx);
                    break;
                }
                // Load specializations
                case OpCode.LOADTRUE: {
                    const destReg = frame.readNext()
                    regs[destReg] = true
                    break
                }
                case OpCode.LOADFALSE: {
                    const destReg = frame.readNext()
                    regs[destReg] = false
                    break
                }
                case OpCode.LOADEMPTYLIST: {
                    const destReg = frame.readNext()
                    regs[destReg] = null // empty list is null
                    break
                }
                case OpCode.LOADVOID: {
                    const destReg = frame.readNext()
                    regs[destReg] = undefined // #<void> is undefined
                    break
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
                    if (!execScope.data.has(varname)) throw new MissingVarError(`Variable '${String(varname)}' is not defined in the current scope.`)
                    regs[destReg] = execScope.data.get(varname)
                    break
                }
                case OpCode.SETGLOBAL: {
                    const srcReg = frame.readNext()
                    const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                    if (execScope.frozen) throw new Error(`Variable '${String(varname)}' cannot be set in a frozen scope.`);
                    execScope.data.set(varname, regs[srcReg])
                    break
                }
                case OpCode.HASGLOBAL: {
                    const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                    if (!execScope.data.has(varname)) throw new MissingVarError(`Variable '${String(varname)}' is not defined in the current scope.`)
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

                case OpCode.CALL: {
                    const proc = regs[frame.readNext()];
                    const destReg = frame.readNext();
                    const startReg = frame.readNext();
                    const nargs = frame.readNext();
                    this.#invoke(proc, frames, frame, regs, destReg, startReg, nargs, false)
                    break;
                }
                case OpCode.TAILCALL: {
                    const proc = regs[frame.readNext()];
                    const startReg = frame.readNext();
                    const nargs = frame.readNext();
                    // Note: we reuse the frames existing return reg for tailcall's
                    this.#invoke(proc, frames, frame, regs, frame.retReg, startReg, nargs, true);
                    break;
                }
                case OpCode.BUILTINCALL: {
                    const proc = IBUILTINS[frame.readNext()];
                    const destReg = frame.readNext();
                    const startReg = frame.readNext();
                    const nargs = frame.readNext();
                    proc.cb(regs, destReg, startReg, nargs)
                    break
                }
                case OpCode.NEWCLOSURE: {
                    const destReg = frame.readNext()
                    const tidx = frame.readNext()
                    const template = frame.getConst(tidx) as ClosureTemplate
                    const closure = new Closure(template)
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
                case OpCode.RETURN: {
                    const retVal = regs[frame.readNext()]; 
                    const finishedFrame = frames.pop();    
                    
                    if (frames.length > 0 && finishedFrame) {
                        const callerFrame = frames[frames.length - 1];
                        callerFrame.regs.regs[finishedFrame.retReg] = retVal;
                    }
                    if (frames.length === 0) return retVal // return retVal back to js
                    break; // back to loop start
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
                default:
                    let _: never = opcode;
            }
        }
    }

    #invoke(proc: any, frames: CallFrame[], frame: CallFrame, regs: any[], destReg: number, startReg: number, nargs: number, isTail: boolean): void {
        if (proc instanceof BuiltinFunction) {
            // Easy case: we have a builtin function!
            proc.cb(regs, destReg, startReg, nargs)
            if (isTail) {
                frames.pop()
                if(frames.length > 0) {
                    frames[frames.length-1].regs.regs[frame.retReg] = regs[destReg]
                }
            }
        } else if (proc instanceof ApplyProc) {
            const actualProc = regs[startReg];

            // create a virtual set of registers to hold the arguments and copy args to it
            const actualArgs = [];
            for (let i = 1; i < nargs - 1; i++) {
                actualArgs.push(regs[startReg + i]);
            }
            const finalArg = regs[startReg + nargs - 1]
            if (Array.isArray(finalArg) || finalArg instanceof Cons) {
                actualArgs.push(...finalArg);
            } else if (finalArg === null) {
                // Empty list
            } else {
                throw new Error(`apply: last argument must be a list but got ${String(finalArg)}`);
            }
            this.#invoke(actualProc, frames, frame, actualArgs, destReg, 0, actualArgs.length, isTail)
            if (!isTail) regs[destReg] = actualArgs[destReg] // copy return value
            return
        } else if (proc instanceof Closure) {
            const template = proc.tmpl;
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

            const closureRegs = new RegBlock(template.code.numReg)

            // required
            for (let i = 0; i < arity; i++) {
                closureRegs.regs[i] = regs[startReg+i]
            }

            // variadic
            if (template.remParams !== null) {
                const restArgs = new Array(nargs-arity);
                for (let i = 0; i < restArgs.length; i++) {
                    restArgs[i] = regs[startReg + arity + i];
                }
                closureRegs.regs[arity] = restArgs
            }

            if (isTail) {
                // Reuse existing frame (TCO)
                frame.code = template.code;
                frame.upvars = proc.upvars
                frame.regs = closureRegs
                frame.ip = 0; 
                frame.retReg = destReg;
            } else {
                frames.push(new CallFrame(template.code, closureRegs, proc.upvars, 0, destReg));
            }
        } else {
            throw new Error(`Attempted to call a non-procedure: ${String(proc)}`);
        }
    }
}