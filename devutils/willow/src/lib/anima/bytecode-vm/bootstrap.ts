import { AnimaScope, OP_ADD, OP_APPLY, OP_DIV, OP_EQ, OP_LIST, OP_MODULO, OP_MUL, OP_REMAINDER, OP_SUB, OP_UI_GET } from "../common";
import { Cons } from "../list";
import { AnimaVM, BuiltinFunction, NativeFunction } from "./vm";

class MapObj {
    #iters: (ArrayIterator<any> | Generator<any, void, unknown>)[]
    #isdone: boolean
    constructor(args: any[]) {
        if (args.length < 2) throw new Error("map requires at least 2 arguments (procedure and 1+ lists to map over)");
        
        const proc = args[0];
        const lists = args.slice(1)
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

const privScope = AnimaScope.newWith({
    [Symbol.for("%MapObj")]: new NativeFunction("%MapObj", -1, (...args) => {
        return new MapObj(args)
    }),
    [Symbol.for("%MapObjDone")]: new NativeFunction("%MapObjDone", 1, (...args) => {
        return args[0].done
    }),
    [Symbol.for("%MapObjNext")]: new NativeFunction("%MapObjNext", 1, (...args) => {
        return args[0].next()
    }),
    [Symbol.for("%ArrayNew")]: new NativeFunction("%ArrayNew", 0, (...args) => {
        return []
    }),
    [Symbol.for("%ArrayPush")]: new NativeFunction("%ArrayPush", 2, (...args) => {
        if(!Array.isArray(args[0])) throw new Error(`internal error: %ArrayPush called on non-array ${args[0]}`)
        args[0].push(args[1])
    }),
    [OP_APPLY]: new BuiltinFunction(-1, false, (bc) => {
        // note to self: its vararg, so nargs is at top of stack. As this is a stub that exists to trigger
        // the intrinsic, the apply is also in tail position, so emit a tail apply instead of apply
        bc.intrinsicTailApply()
    }),
    [OP_ADD]: new BuiltinFunction(-1, false, (bc) => {
        // note to self: its vararg, so nargs is at top of stack
        bc.intrinsicAdd()
    }),
    [OP_SUB]: new BuiltinFunction(-1, false, (bc) => {
        // note to self: its vararg, so nargs is at top of stack
        bc.intrinsicSub()
    }),
    [OP_MUL]: new BuiltinFunction(-1, false, (bc) => {
        // note to self: its vararg, so nargs is at top of stack
        bc.intrinsicMul()
    }),
    [OP_DIV]: new BuiltinFunction(-1, false, (bc) => {
        // note to self: its vararg, so nargs is at top of stack
        bc.intrinsicDiv()
    }),
    [OP_MODULO]: new BuiltinFunction(2, false, (bc) => {
        bc.intrinsicModulo()
    }),
    [OP_REMAINDER]: new BuiltinFunction(2, false, (bc) => {
        bc.intrinsicRemainder()
    }),
    [OP_LIST]: new BuiltinFunction(-1, false, (bc) => {
        bc.intrinsicList()
    }),
    [OP_UI_GET]: new BuiltinFunction(-1, false, (bc) => {
        bc.intrinsicUiGet()
    }),
    [OP_EQ]: new BuiltinFunction(-1, false, (bc) => {
        bc.intrinsicEq()
    }),
}, false);

const bootstrapVM = new AnimaVM();
bootstrapVM.evaluateStr(`
(define (map f . lists)
    (let ((iter (apply %MapObj f lists))
          (result (%ArrayNew)))
    
    (let loop ()
      (let ((args (%MapObjNext iter)))      
        (if (%MapObjDone iter)              
            result                          
            (begin
                (%ArrayPush result (apply f args)) 
                (loop)))))))    
`, privScope)

const publicScope = AnimaScope.new(false); 
for (const [sym, value] of privScope.entries()) {
    const symName = Symbol.keyFor(sym) || sym.description || "%Unknown";
    
    // Drop prelude fns in the public root scope
    if (!symName.startsWith("%")) {
        publicScope.define(sym, value);
    }
}
publicScope.setFrozen(true)

export function createScope(): AnimaScope {
    return publicScope.nest();
}