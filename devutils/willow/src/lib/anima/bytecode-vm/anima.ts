import { AnimaScope, ExposedProps } from "../common";
import { createScope } from "./bootstrap";
import { AnimaCompiler } from "./compiler";
import { AnimaVM, BuiltinFunction, ByteCode, Closure, ClosureTemplate, NativeFunction } from "./vm";

interface AnimaOpts {
    disableLambda?: boolean,
    disableDefine?: boolean,
    disableSet?: boolean,
    maxSteps?: number
}

export class Anima {
    #vm: AnimaVM
    #comp: AnimaCompiler
    #scope: AnimaScope
    #disableLambda: boolean
    #disableDefine: boolean
    #disableSet: boolean

    get rootScope() {
        return this.#scope
    }

    constructor(opts?: AnimaOpts) {
        this.#vm = new AnimaVM(0, opts?.maxSteps ? opts.maxSteps : 0)
        this.#comp = new AnimaCompiler()
        this.#scope = createScope()
        this.#disableLambda = opts?.disableLambda || false
        this.#disableDefine = opts?.disableDefine || false
        this.#disableSet = opts?.disableSet || false
    }

    public compileStr(expr: string): ByteCode {
        return this.#comp.compileStr(expr, this.#disableDefine, this.#disableLambda, this.#disableSet)
    }

    public evaluate(code: ByteCode, props?: ExposedProps): any {
        return this.#vm.evaluate(code, this.#scope, props);
    }
}

// Re-export
export { AnimaCompiler, ByteCode, AnimaScope, ExposedProps, ClosureTemplate, NativeFunction, BuiltinFunction, Closure }