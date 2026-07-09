import { BS, BSReader, ErrorObject, ExposedProps, IProcedure, isDeepEqual, MissingVarError, OP_ADD, OP_CAR, OP_CDR, OP_CONS, OP_CONTAINS, OP_DIV, OP_EMPTY, OP_EQ, OP_EQQ, OP_EQUAL, OP_EQV, OP_GT, OP_GTE, OP_LAST, OP_LENGTH, OP_LIST, OP_LT, OP_LTE, OP_MEMBER, OP_MODULO, OP_MUL, OP_NOT, OP_REMAINDER, OP_SUB, OP_TYPE, OP_UI_GET, type SerializableBytecode } from "../common";
import { isTruthy } from "../common";
import { Cons } from "../list";

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

/** 
 * A builtin function. 
 * 
 * Builtin functions do not have access to their own lexical scope (at least not yet) 
 * 
 * It is undefined behaviour for a builtin function to modify regs (reg's are considered readonly). Additionally, the intermediate
 * state of any BuiltinFunction must be well-defined/valid 
*/
export class BuiltinFunction extends IProcedure {
    constructor(
        public name: symbol,
        public cb: (regs: readonly any[], startReg: number, nargs: number) => any,
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
            const iterator = val[Symbol.iterator]();
            let result = iterator.next();
            let last = result.value;

            while (!result.done) {
                last = result.value;
                result = iterator.next();
            }

            if (result.value !== undefined) {
                last = result.value;
            }
            return last;
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
    new BuiltinFunction(Symbol.for("error"), (regs, startReg, nargs) => {
        if (nargs != 1) throw new Error("");
        throw new Error(regs[startReg])
    }),
    new BuiltinFunction(Symbol.for("error?"), (regs, startReg, nargs) => {
        if (nargs != 1) throw new Error("error? requires 1 argument");
        return regs[startReg] instanceof ErrorObject
    }),
    new BuiltinFunction(Symbol.for("error-message"), (regs, startReg, nargs) => {
        if (nargs != 1) throw new Error("error-message requires 1 argument");
        if(!(regs[startReg] instanceof ErrorObject)) throw new Error("error-message requires the first argument to be an instance of ErrorObject")
        return regs[startReg].error?.message?.toString() || "<unknown>"
    }),
    new BuiltinFunction(OP_TYPE, (regs, startReg, nargs) => {
        if (nargs != 1) throw new Error("type? requires 1 argument");
        const val = regs[startReg]
        if (val === null) return "list";
        switch(typeof val) {
            case "string": return "string"
            case "number": return "number"
            case "boolean": return "boolean"
            case "undefined": return "null"
            case "symbol": return "symbol";
            default: {
                if (val instanceof IProcedure) return "procedure";
                if(Array.isArray(val) || val instanceof Cons) return "list"
                if (val instanceof ErrorObject) return "error";
                if (val instanceof ExposedProps) return "exposed-props";
                return "object" // to allow consistency across all js engines/custom sv2 impls etc.
            }
        }
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

export const IBUILTINS_IDX_MAP = new Map<symbol, number>()
for(let i = 0; i < IBUILTINS.length; i++) {
    IBUILTINS_IDX_MAP.set(IBUILTINS[i].name, i)
}

// Marker for `apply` intrinsic proc
export class ApplyProc extends IProcedure {}
export const APPLY_PROC = new ApplyProc()
// Marker for `try` intrinsic proc
export class TryProc extends IProcedure {}
export const TRY_PROC = new TryProc()
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

export type ExceptionHandler = {
    returnCont: any;     // k 
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
        cont: Continuation,
        callerFrame: CallFrame, 
        callerArgs: any[], 
        startReg: number, 
        nargs: number
    ): Continuation {
        if (cont.type !== "RUNNING") throw new Error("internal error: cannot invoke function using non-running cont")
        if (proc instanceof BuiltinFunction) {
            const contArg = callerArgs[startReg];
            const retVal = proc.cb(callerArgs, startReg+1, nargs-1)
            return this.#invoke(contArg, cont, callerFrame, [retVal], 0, 1);
        } else if (proc instanceof ApplyProc) {
            const contArg = callerArgs[startReg];
            const actualProc = callerArgs[startReg+1];

            // create a virtual set of registers to hold the arguments and copy args to it
            const actualArgs = [contArg];
            for (let i = 2; i < nargs - 1; i++) {
                actualArgs.push(callerArgs[startReg + i]);
            }
            const finalArg = callerArgs[startReg + nargs - 1]
            if (Array.isArray(finalArg) || finalArg instanceof Cons) {
                actualArgs.push(...finalArg);
            } else if (finalArg !== null) {
                throw new Error(`apply: last argument must be a list but got ${String(finalArg)}`);
            }
            return this.#invoke(actualProc, cont, callerFrame, actualArgs, 0, actualArgs.length);
        } else if (proc instanceof TryProc) {
            const contArg = callerArgs[startReg];
            const actualProc = callerArgs[startReg+1];

            const newHandler: ExceptionHandler = {
                returnCont: contArg,
                parent: cont.handler
            };
            const successProc = new TrySuccessProc(contArg, cont.handler);

            // create a virtual set of registers to hold the arguments and copy args to it
            const actualArgs = [successProc];
            for (let i = 2; i < nargs - 1; i++) {
                actualArgs.push(callerArgs[startReg + i]);
            }
            const finalArg = callerArgs[startReg + nargs - 1]
            if (Array.isArray(finalArg) || finalArg instanceof Cons) {
                actualArgs.push(...finalArg);
            } else if (finalArg !== null) {
                throw new Error(`try: last argument must be a list but got ${String(finalArg)}`);
            }

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