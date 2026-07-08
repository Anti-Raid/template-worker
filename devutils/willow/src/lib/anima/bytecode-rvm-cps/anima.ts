import { ASP, ASTStringifier, DottedPair, ExposedProps, OP_LAMBDA } from "../common";
import { AnimaTransformer } from "../syntax-transformer";
import { publicScope } from "./bootstrap";
import { Compiler } from "./compiler";
import { AnimaVM, BuiltinFunction, ByteCode, Closure, ClosureTemplate, Globals } from "./vm";

interface AnimaOpts {
    maxSteps?: number
}

export class Anima {
    #vm: AnimaVM
    #comp: Compiler
    #scope: Globals

    get scope() {
        return this.#scope
    }

    get compiler() {
        return this.#comp
    }

    get vm() {
        return this.#vm
    }

    constructor(opts?: AnimaOpts) {
        this.#vm = new AnimaVM(0, opts?.maxSteps ? opts.maxSteps : 0)
        this.#comp = new Compiler()
        this.#scope = publicScope.nestWith({})
    }

    public evaluateRaw(code: ByteCode): any {
        return this.#vm.evaluateRaw(code, this.#scope)
    }

    public evaluateClosure(code: Closure, args: any[]): any {
        return this.#vm.evaluateClosure(code, this.#scope, args)
    }

    compileToClosure(s: string, args: any[] | DottedPair, globals: Globals) {
        const bast = new ASP(s, true).parse()
        return this.compileAstToClosure(bast, args, globals)
    }

    compileAstToClosure(bast: any, args: any[] | DottedPair, globals: Globals) {
        const ast = [OP_LAMBDA, args, bast]
        const bc = this.#comp.compileRawAst(ast)
        const res = this.#vm.evaluateRaw(bc, globals) // Use the VM to create the closure
        if (!(res instanceof Closure)) throw new Error("internal error: compileToClosure did not return a closure")
        return res
    }

    compileRaw(s: string) {
        return this.#comp.compileRaw(s)
    }

    compileRawAst(ast: any) {
        return this.#comp.compileRawAst(ast)
    }
}

// Re-export
export { Compiler, AnimaVM, ByteCode, Globals, ExposedProps, ClosureTemplate, BuiltinFunction, Closure, ASTStringifier, AnimaTransformer, DottedPair, ASP }