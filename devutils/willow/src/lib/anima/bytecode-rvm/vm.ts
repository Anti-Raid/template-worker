import { ExposedProps, IProcedure, isDeepEqual, MissingVarError, OP_ADD, OP_CAR, OP_CDR, OP_CONS, OP_CONTAINS, OP_DIV, OP_EMPTY, OP_EQ, OP_EQQ, OP_EQUAL, OP_EQV, OP_GT, OP_GTE, OP_LAST, OP_LENGTH, OP_LIST, OP_LT, OP_LTE, OP_MEMBER, OP_MODULO, OP_MUL, OP_NOT, OP_REMAINDER, OP_SUB, OP_UI_GET } from "../common";
import { isTruthy } from "../common";
import { Cons } from "../list";

export const BUILTINS_START = 2**31
export const APPLY_PROC_IDX = 2**32 - 1

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
    RETURN,
    NEWCLOSURE,
    BOX,
    UNBOX,
    SETBOX,
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
        public cb: (regs: any[], startReg: number, nargs: number) => any,
    ) {
        super()
    }
}

// Stores all of our builtin funcs
export const IBUILTINS: BuiltinFunction[] = [
    new BuiltinFunction(OP_ADD, (regs, startReg, nargs) => {
        let acc = 0; 
        for (let i = startReg; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`+ requires numbers, but received ${typeof val}`);
            acc += val
        }
        return acc
    }),
    new BuiltinFunction(OP_SUB, (regs, startReg, nargs) => {
        if (nargs === 0) throw new Error("- requires at least 1 argument");
        
        if (nargs === 1) {
            const val = regs[startReg];
            if (typeof val !== "number") throw new Error(`- requires numbers, but received ${typeof val}`);
            return -val; 
        }

        let acc = regs[startReg];
        if (typeof acc !== "number") throw new Error(`- requires numbers, but received ${typeof acc}`);
        for (let i = startReg + 1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`- requires numbers, but received ${typeof val}`);
            acc -= val
        }
        return acc
    }),
    new BuiltinFunction(OP_MUL, (regs, startReg, nargs) => {
        let acc = 1; 
        for (let i = startReg; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`* requires numbers, but received ${typeof val}`);
            acc *= val
        }
        return acc
    }),
    new BuiltinFunction(OP_DIV, (regs, startReg, nargs) => {
        if (nargs === 0) throw new Error("/ requires at least 1 argument");
        
        if (nargs === 1) {
            const val = regs[startReg];
            if (typeof val !== "number") throw new Error(`/ requires numbers, but received ${typeof val}`);
            if (val === 0) throw new Error("division by zero");
            return 1/val; 
        }

        let acc = regs[startReg];
        if (typeof acc !== "number") throw new Error(`/ requires numbers, but received ${typeof acc}`);
        for (let i = startReg + 1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (typeof val !== "number") throw new Error(`/ requires numbers, but received ${typeof val}`);
            if (val === 0) throw new Error("division by zero");
            acc /= val
        }
        return acc
    }),
    new BuiltinFunction(OP_MODULO, (regs, startReg, nargs) => {
        if(nargs !== 2) throw new Error("modulo requires 2 arguments");
        const a = regs[startReg] 
        const b = regs[startReg+1]
        if (typeof a !== "number" || typeof b !== "number") throw new Error(`modulo: requires numbers, but received ${typeof a}/${typeof b}`);
        if (b === 0) throw new Error("modulo: division by zero");
        return ((a % b) + b) % b
    }),
    new BuiltinFunction(OP_REMAINDER, (regs, startReg, nargs) => {
        if(nargs !== 2) throw new Error("remainder requires 2 arguments");
        const a = regs[startReg] 
        const b = regs[startReg+1]
        if (typeof a !== "number" || typeof b !== "number") throw new Error(`remainder: requires numbers, but received ${typeof a}/${typeof b}`);
        if (b === 0) throw new Error("remainder: division by zero");
        return a % b
    }),
    new BuiltinFunction(OP_LIST, (regs, startReg, nargs) => {
        const lst = new Array(nargs)
        for(let i = 0; i < nargs; i++) {
            lst[i] = regs[startReg+i]
        }
        return lst
    }),
    new BuiltinFunction(OP_EQ, (regs, startReg, nargs) => {
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
        return res
    }),
    new BuiltinFunction(OP_EQQ, (regs, startReg, nargs) => {
        // DEVIATION: normal scheme requires arity 2, anima extends this to arity >=1
        if (nargs === 0) throw new Error("eq? requires at least 1 argument");
        
        let start = regs[startReg];
        let res = true
        for (let i = startReg+1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (val !== start) {
                res = false
                break
            }
        }
        return res
    }),
    new BuiltinFunction(OP_EQV, (regs, startReg, nargs) => {
        // DEVIATION: normal scheme requires arity 2, anima extends this to arity >=1
        if (nargs === 0) throw new Error("eqv? requires at least 1 argument");
        
        let start = regs[startReg];
        let res = true
        for (let i = startReg+1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (!Object.is(val, start)) {
                res = false
                break
            }
        }
        return res
    }),
    new BuiltinFunction(OP_EQUAL, (regs, startReg, nargs) => {
        if (nargs === 0) throw new Error("equal? requires at least 1 argument");
        
        let start = regs[startReg];
        let res = true
        for (let i = startReg+1; i < startReg+nargs; i++) {
            const val = regs[i]
            if (!isDeepEqual(val, start)) {
                res = false
                break
            }
        }
        return res
    }),
    new BuiltinFunction(OP_LT, (regs, startReg, nargs) => {
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
        return res
    }),
    new BuiltinFunction(OP_LTE, (regs, startReg, nargs) => {
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
        return res
    }),
    new BuiltinFunction(OP_GT, (regs, startReg, nargs) => {
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
        return res
    }),
    new BuiltinFunction(OP_GTE, (regs, startReg, nargs) => {
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
        return res
    }),
    // list builtins
    new BuiltinFunction(OP_CAR, (regs, startReg, nargs) => {
        if (nargs !== 1) throw new Error("car requires 1 argument");
        const val = regs[startReg]
        if (Array.isArray(val)) {
            if (val.length < 1) throw new Error("car requires a non-empty list");
            return val[0];
        } else if (val instanceof Cons) {
            return val.head;
        } else if (val === null) {
            throw new Error("car requires a non-empty list");
        } else {
            throw new Error("car requires a list");
        }
    }),
    new BuiltinFunction(OP_CDR, (regs, startReg, nargs) => {
        if (nargs !== 1) throw new Error("cdr requires 1 argument");
        const val = regs[startReg]
        if (Array.isArray(val)) {
            if (val.length < 1) throw new Error("cdr requires a non-empty list");
            return Cons.fromArray(val, 1)
        } else if (val instanceof Cons) { 
            return val.tail
        } else if (val === null) {
            throw new Error("cdr requires a non-empty list");
        } else {
            throw new Error(`cdr requires a list but got ${val}`);
        }
    }),
    new BuiltinFunction(OP_CONS, (regs, startReg, nargs) => {
        if (nargs != 2) throw new Error("cons requires 2 arguments [cons a d]");
        return Cons.pair(regs[startReg], regs[startReg+1])
    }),
    new BuiltinFunction(OP_LAST, (regs, startReg, nargs) => {
        if (nargs != 1) throw new Error("last requires 1 argument");
        const val = regs[startReg]
        if (Array.isArray(val)) {
            if (val.length < 1) throw new Error("last requires a non-empty list");
            return val[val.length - 1];
        } else if (val instanceof Cons) {
            let last = val.head
            for(const v of val) {
                last = v;
            }
            return last
        } else if (val === null) {
            throw new Error("last requires a non-empty list");
        } else {
            throw new Error("last requires a list");
        }
    }),
    new BuiltinFunction(OP_LENGTH, (regs, startReg, nargs) => {
        if (nargs != 1) throw new Error("length requires 1 argument");
        const val = regs[startReg]
        if (val === null) {
            return 0 // empty list
        }
        // TODO: Add string-length? for strings specifically like scheme does
        return (Array.isArray(val) || val instanceof Cons) ? val.length : (typeof val === "string" ? val.length : 0)
    }),
    new BuiltinFunction(OP_EMPTY, (regs, startReg, nargs) => {
        if (nargs != 1) throw new Error("empty requires 1 argument");
        const val = regs[startReg]
        if (val === null) {
            return true // empty list
        }
        return (Array.isArray(val) || val instanceof Cons) ? (val.length == 0) : (typeof val === "string" ? (val.length == 0) : false);
    }),
    new BuiltinFunction(OP_CONTAINS, (regs, startReg, nargs) => {
        if (nargs != 2) throw new Error("contains? requires 2 arguments");
        const list = regs[startReg]
        const item = regs[startReg+1]
        return (Array.isArray(list) || list instanceof Cons) ? list.includes(item) : false;
    }),
    new BuiltinFunction(OP_MEMBER, (regs, startReg, nargs) => {
        if (nargs != 2) throw new Error("member? requires 2 arguments");
        let list = regs[startReg] as (any[] | Cons | null)
        const item = regs[startReg+1]
        if (Array.isArray(list)) {
            for (let i = 0; i < list.length; i++) {
                if (isDeepEqual(list[i], item)) {
                    // create a view of the array starting from i
                    return Cons.fromArray(list, i)
                }
            }
            return false;
        }
        else if (list === null) {
            return false // not a member if empty list
        }
        else if (!(list instanceof Cons)) throw new Error("member? requires the first argument to be a list")
        list = list as Cons // cast to Cons for type safety
        let s: Cons | null = list
        while(s !== null) {
            if (isDeepEqual(s.head, item)) {
                return s
            }
            s = s.tail
        }
        return false
    }),
    new BuiltinFunction(OP_NOT, (regs, startReg, nargs) => {
        if (nargs != 1) throw new Error("not requires 1 argument");
        return !isTruthy(regs[startReg]);
    }),
    new BuiltinFunction(Symbol.for("display"), (regs, startReg, nargs) => {
        if (nargs != 1) throw new Error("display requires 1 argument");
        console.log(regs[startReg])
        return undefined
    }),
    new BuiltinFunction(OP_UI_GET, (regs, startReg, nargs) => {
        if (nargs != 2) throw new Error("ui-get requires 2 arguments (ui-get props key-str)");
        const props = regs[startReg]
        if (!(props instanceof ExposedProps)) throw new Error("ui-get requires the first argument to be an instance of ExposedProps")
        const keyStr = regs[startReg+1]
        if (typeof keyStr !== "string") throw new Error("ui-get requires the second argument to be a string")
        return props.get(keyStr)
    })
]

// Marker for `apply` intrinsic proc
export class ApplyProc extends IProcedure {}
export const APPLY_PROC = new ApplyProc()

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

export class Globals {
    private constructor(public data: Map<symbol, any>, public frozen: boolean = false, public outer: Globals | null) {}

    static newWith(fields: Record<symbol, any>, frozen: boolean = false) {
        const map = new Map()
        Object.getOwnPropertySymbols(fields).forEach((sym) => {
            map.set(sym, fields[sym])
        });
        return new Globals(map, frozen, null);
    }

    nestWith(fields: Record<symbol, any>, frozen: boolean = false) {
        const map = new Map()
        Object.getOwnPropertySymbols(fields).forEach((sym) => {
            map.set(sym, fields[sym])
        });
        return new Globals(map, frozen, this);
    }

    get(varname: symbol): any {
        if (this.data.has(varname)) {
            return this.data.get(varname)
        }
        if (this.outer) {
            return this.outer.get(varname)
        }
        throw new MissingVarError(`Variable '${String(varname)}' is not defined in the current scope.`)
    }

    assert(varname: symbol): void {
        if (this.data.has(varname)) {
            return
        }
        if (this.outer) {
            return this.outer.assert(varname)
        }
        throw new MissingVarError(`Variable '${String(varname)}' is not defined in the current scope.`)
    }

    set(varname: symbol, data: any) {
        if (this.frozen) throw new Error(`Variable '${String(varname)}' cannot be set in a frozen scope.`);
        this.data.set(varname, data)
    }
}

type Continuation = { type: 'RUNNING'; frame: CallFrame; parent: Continuation | null, destReg?: number }
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
        const cargs = this.#createClosureArg(code, args.length, args, 0)
        let frame: CallFrame = new CallFrame(code.tmpl.code, cargs, code.upvars, 0, 0);
        try {
            return this.#execnext(frame, scope);
        } catch (err: any) {
            console.log(`${err.stack}\n\nCurrent Frame IP: ${frame.ip}`)
            throw err
        }
    }

    #execnext(initialFrame: CallFrame, execScope: Globals) {
        let cont: Continuation = { type: 'RUNNING', frame: initialFrame, parent: null };
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
                    const proc = (procIdx === APPLY_PROC_IDX) ? APPLY_PROC : (procIdx < BUILTINS_START) ? regs[procIdx] : IBUILTINS[procIdx - BUILTINS_START];
                    const destReg = frame.readNext();
                    const startReg = frame.readNext();
                    const nargs = frame.readNext();
                    cont = this.#invoke(proc, cont, frame, regs, destReg, startReg, nargs)
                    break;
                }
                case OpCode.TAILCALL: {
                    const procIdx = frame.readNext()
                    const proc = (procIdx === APPLY_PROC_IDX) ? APPLY_PROC : (procIdx < BUILTINS_START) ? regs[procIdx] : IBUILTINS[procIdx - BUILTINS_START];
                    const startReg = frame.readNext();
                    const nargs = frame.readNext();
                    cont = this.#invoke(proc, cont, frame, regs, undefined, startReg, nargs);
                    break;
                }
                default:
                    let _: never = opcode;
            }
        }

        return cont.value
    }

    // Note: if destReg is not set, we treat it as a tailcall
    #invoke(
        proc: any, 
        cont: Continuation,
        callerFrame: CallFrame, 
        callerArgs: any[], 
        destReg: number | undefined, 
        startReg: number, 
        nargs: number
    ): Continuation {
        if (cont.type !== "RUNNING") throw new Error("internal error: cannot invoke function using non-running cont")
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

            // create a virtual set of registers to hold the arguments and copy args to it
            const actualArgs = [];
            for (let i = 1; i < nargs - 1; i++) {
                actualArgs.push(callerArgs[startReg + i]);
            }
            const finalArg = callerArgs[startReg + nargs - 1]
            if (Array.isArray(finalArg) || finalArg instanceof Cons) {
                actualArgs.push(...finalArg);
            } else if (finalArg === null) {
                // Empty list
            } else {
                throw new Error(`apply: last argument must be a list but got ${String(finalArg)}`);
            }
            return this.#invoke(actualProc, cont, callerFrame, actualArgs, destReg, 0, actualArgs.length)
        } else if (proc instanceof Closure) {
            const template = proc.tmpl;
            const pregs = this.#createClosureArg(proc, nargs, callerArgs, startReg)
            const parentCont = (destReg === undefined) ? cont.parent : {
                type: 'RUNNING',
                frame: callerFrame,
                parent: cont.parent,
                destReg: destReg 
            } as Continuation | null;
            const nextFrame = new CallFrame(template.code, pregs, proc.upvars, 0, callerFrame.id+1);
            return { type: 'RUNNING', frame: nextFrame, parent: parentCont };
        } else {
            throw new Error(`Attempted to call a non-procedure: ${String(proc)}`);
        }
    }

    #createClosureArg(proc: Closure, nargs: number, args: any[], startOffset: number) {
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
