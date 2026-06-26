import { OP_ADD, OP_APPLY, OP_DIV, OP_EQ, OP_LIST, OP_MODULO, OP_MUL, OP_REMAINDER, OP_SUB, OP_UI_GET } from "../common";
import { Cons } from "../list";
import { AnimaCompiler } from "./compiler";
import { AnimaVM, BuiltinFunction, ByteCode, ClosureTemplate, Globals, NativeFunction, OpCode } from "./vm";

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

const newBc = (initializer: (bc: number[]) => void) => {
    const bc: number[] = []
    initializer(bc)
    return new ByteCode([], bc)
}

const privScope = Globals.newWith({
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
    [OP_APPLY]: new BuiltinFunction(-1, newBc((bc) => {
        // note to self: its vararg, so nargs is at top of stack. As this is a stub that exists to trigger
        // the intrinsic, the apply is also in tail position, so emit a tail apply instead of apply
        bc.push(OpCode.INTRINSIC_TAIL_APPLY)
    })),
    [OP_ADD]: new BuiltinFunction(-1, newBc((bc) => {
        // note to self: its vararg, so nargs is at top of stack
        bc.push(OpCode.INTRINSIC_ADD)
    })),
    [OP_SUB]: new BuiltinFunction(-1, newBc((bc) => {
        // note to self: its vararg, so nargs is at top of stack
        bc.push(OpCode.INTRINSIC_SUB)
    })),
    [OP_MUL]: new BuiltinFunction(-1, newBc((bc) => {
        // note to self: its vararg, so nargs is at top of stack
        bc.push(OpCode.INTRINSIC_MUL)
    })),
    [OP_DIV]: new BuiltinFunction(-1, newBc((bc) => {
        // note to self: its vararg, so nargs is at top of stack
        bc.push(OpCode.INTRINSIC_DIV)
    })),
    [OP_MODULO]: new BuiltinFunction(2, newBc((bc) => {
        bc.push(OpCode.INTRINSIC_MODULO)
    })),
    [OP_REMAINDER]: new BuiltinFunction(2, newBc((bc) => {
        bc.push(OpCode.INTRINSIC_REMAINDER)
    })),
    [OP_LIST]: new BuiltinFunction(-1, newBc((bc) => {
        bc.push(OpCode.INTRINSIC_LIST)
    })),
    [OP_UI_GET]: new BuiltinFunction(-1, newBc((bc) => {
        bc.push(OpCode.INTRINSIC_UI_GET)
    })),
    [OP_EQ]: new BuiltinFunction(-1, newBc((bc) => {
        bc.push(OpCode.INTRINSIC_EQ)
    })),
}, false);

const PRELUDE = `
(define create-map (lambda ()
    ; copy in prelude as locals to the lambda
    (define %MapObj_ %MapObj)
    (define %ArrayNew_ %ArrayNew)
    (define %ArrayPush_ %ArrayPush)
    (define %MapObjNext_ %MapObjNext)
    (define %MapObjDone_ %MapObjDone)
    (lambda (f . lists)
        (let ((iter (apply %MapObj_ f lists))
            (result (%ArrayNew_)))
        
        (let loop ()
        (let ((args (%MapObjNext_ iter)))      
            (if (%MapObjDone_ iter)              
                result                          
                (begin
                    (%ArrayPush_ result (apply f args)) 
                    (loop)))))))))

(define map (create-map))
`

const bootstrapVM = new AnimaVM();
const bootstrapComp = new AnimaCompiler()
const PRELUDE_BC = bootstrapComp.compileStr(PRELUDE)

console.log(PRELUDE_BC.constants[0].code.toString())
for (let i = 0; i < PRELUDE_BC.constants.length; i++) {
    const c = PRELUDE_BC.constants[i]
    if (c instanceof ClosureTemplate) {
        console.log(`Const #${i}:\n${c.code.toString()}`)
    }
}

bootstrapVM.evaluate(PRELUDE_BC, privScope)


const publicScope = Globals.newWith({}, false); 
for (const [sym, value] of privScope.data.entries()) {
    const symName = Symbol.keyFor(sym) || sym.description || "%Unknown";
    
    // Drop prelude fns in the public root scope
    if (!symName.startsWith("%")) {
        publicScope.data.set(sym, value);
    }
}

export function createScope(): Globals {
    return publicScope
}