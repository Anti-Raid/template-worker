import { OP_APPLY, OP_TRY } from "../common";
import { Cons } from "../list";
import { Compiler } from "./compiler";
import { AnimaVM, APPLY_PROC, BuiltinFunction, Globals, IBUILTINS, TRY_PROC } from "./vm";

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

const privScope = Globals.newWith({
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
    [OP_APPLY]: APPLY_PROC, // technically not needed due to compiler optimizing intrinsics directly but for correctness purposes, keep it
    [OP_TRY]: TRY_PROC,
})

const PRELUDE = `
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

const bootstrapVM = new AnimaVM();
const bootstrapComp = new Compiler()
const PRELUDE_BC = bootstrapComp.compileRaw(PRELUDE)
bootstrapVM.evaluateRaw(PRELUDE_BC, privScope)

/* Base scope */
export const publicScope = Globals.newWith({}, true); 
for (const [sym, value] of privScope.data.entries()) {
    const symName = Symbol.keyFor(sym) || sym.description || "%Unknown";
    
    // If the func starts with a $, its public
    if (symName.startsWith("$")) {
        publicScope.data.set(Symbol.for(symName.replace('$', '')), value);
    }
}

// finally, export the builtins
for(const builtin of IBUILTINS) {
  publicScope.data.set(builtin.name, builtin)
}
publicScope.data.set(OP_APPLY, APPLY_PROC)
publicScope.data.set(OP_TRY, TRY_PROC)
