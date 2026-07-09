import { ErrorObject, ExposedProps, Globals, IProcedure, isDeepEqual, isTruthy, OP_ADD, OP_APPLY, OP_CAR, OP_CDR, OP_CONS, OP_CONTAINS, OP_DIV, OP_EMPTY, OP_EQ, OP_EQQ, OP_EQUAL, OP_EQV, OP_GT, OP_GTE, OP_LAST, OP_LENGTH, OP_LIST, OP_LT, OP_LTE, OP_MEMBER, OP_MODULO, OP_MUL, OP_NOT, OP_REMAINDER, OP_SUB, OP_TRY, OP_TYPE, OP_UI_GET } from "./common";
import { Cons } from "./list";

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

class MapObj {
    #iters: (ArrayIterator<any> | Generator<any, void, unknown>)[]
    #isdone: boolean
    constructor(lists: any[]) {        
        const iters = lists.map(list => {
            if (list === null) return [][Symbol.iterator]();
            if (Array.isArray(list) || list instanceof Cons) return list[Symbol.iterator]();
            throw new Error("map arguments must be lists");
        });
        this.#iters = iters
        this.#isdone = false
    }

    get done() { return this.#isdone }

    next() {
        if (this.#isdone) throw new Error("internal error: map iter done but %MapObjNext still called")
        const nextVals = this.#iters.map(it => it.next());
            
        if (nextVals.some(res => res.done)) {
            this.#isdone = true
            return
        }

        // Unlike cons, this avoids the allocation of O(N) extra cons array views making it more performant
        const args = [];
        for (const res of nextVals) {
            args.push(res.value);
        }

        return args
    }
}

const MAPOBJ = Symbol.for("%MapObj")
const MAPOBJDONE = Symbol.for("%MapObjDone")
const MAPOBJNEXT = Symbol.for("%MapObjNext")
const ARRAYNEW = Symbol.for("%ArrayNew")
const ARRAYPUSH = Symbol.for("%ArrayPush")

export const stdPreludeScope = () => Globals.newWith({
    [MAPOBJ]: new BuiltinFunction(MAPOBJ, (regs, startReg, nargs) => {
        if (nargs < 1) throw new Error("%MapObj requires at least 1 argument (1+ lists to map over)");
        const lists = regs.slice(startReg, startReg+nargs)
        return new MapObj(lists)
    }),
    [MAPOBJDONE]: new BuiltinFunction(MAPOBJDONE, (regs, startReg, nargs) => {
        if (nargs !== 1) throw new Error("%MapObjDone requires 1 argument");
        return regs[startReg].done
    }),
    [MAPOBJNEXT]: new BuiltinFunction(MAPOBJNEXT, (regs, startReg, nargs) => {
        if (nargs !== 1) throw new Error("%MapObjNext requires 1 argument");
        return regs[startReg].next()
    }),
    [ARRAYNEW]: new BuiltinFunction(ARRAYNEW, (_regs, _startReg, nargs) => {
        if (nargs !== 0) throw new Error("%ArrayNew requires 0 arguments");
        return []
    }),
    [ARRAYPUSH]: new BuiltinFunction(ARRAYPUSH, (regs, startReg, nargs) => {
        if (nargs !== 2) throw new Error("%ArrayPush requires 2 arguments");
        if(!Array.isArray(regs[startReg])) throw new Error(`internal error: %ArrayPush called on non-array ${regs[startReg]}`)
        return regs[startReg].push(regs[startReg+1])
    }),
    [OP_APPLY]: APPLY_PROC,
    [OP_TRY]: TRY_PROC,
})

export const STD_PRELUDE = `
(define create-map (lambda ()
    ; copy in prelude as locals to the lambda
    (define %MapObj_ %MapObj)
    (define %ArrayNew_ %ArrayNew)
    (define %ArrayPush_ %ArrayPush)
    (define %MapObjNext_ %MapObjNext)
    (define %MapObjDone_ %MapObjDone)
    (lambda (f . lists)
        (let ((iter (apply %MapObj_ lists))
            (result (%ArrayNew_)))
        
        (let loop ()
        (let ((args (%MapObjNext_ iter)))      
            (if (%MapObjDone_ iter)              
                result                          
                (begin
                    (%ArrayPush_ result (apply f args)) 
                    (loop)))))))))

(define $map (create-map))
`
