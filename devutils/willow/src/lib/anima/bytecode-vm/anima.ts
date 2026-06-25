import { AnimaScope, ExposedProps } from "../common";
import { createScope } from "./bootstrap";
import { AnimaCompiler } from "./compiler";
import { AnimaVM, BuiltinFunction, ByteCode, Closure, ClosureTemplate, NativeFunction } from "./vm";

export class Anima {
    #vm: AnimaVM
    #scope: AnimaScope
    #exposedProps?: ExposedProps

    get rootScope() {
        return this.#scope
    }

    constructor(props?: ExposedProps) {
        this.#vm = new AnimaVM()
        this.#scope = createScope()
        this.#exposedProps = props
    }

    public evaluate(code: ByteCode): any {
        return this.#vm.evaluate(code, this.#scope);
    }

    public evaluateExpr(expr: any, disableDefine: boolean = false, disableLambda: boolean = false): any {
        return this.#vm.evaluateExpr(expr, disableDefine, disableLambda, this.#scope);
    }

    public evaluateStr(expr: string, disableDefine: boolean = false, disableLambda: boolean = false): any {
        return this.#vm.evaluateStr(expr, this.#scope, disableDefine, disableLambda);
    }
}

// Re-export
export { AnimaCompiler, ByteCode, AnimaScope, ExposedProps, ClosureTemplate, NativeFunction, BuiltinFunction, Closure }