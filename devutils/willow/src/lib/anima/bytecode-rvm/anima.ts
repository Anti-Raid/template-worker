import { ASTStringifier, ExposedProps } from "../common";
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
}

// Re-export
export { Compiler, AnimaVM, ByteCode, Globals, ExposedProps, ClosureTemplate, BuiltinFunction, Closure, ASTStringifier, AnimaTransformer }