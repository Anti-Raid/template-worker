import { ASP, Globals, OP_LAMBDA, type DottedPair, type SerializableBytecode } from "./common"
import { IBUILTINS, STD_PRELUDE, stdPreludeScope } from "./std"
import { AnimaTransformer } from "./syntax-transformer"

export interface Closure {}
export interface ByteCode extends SerializableBytecode {}
export interface AnimaVM {
    evaluateRaw(code: ByteCode, scope: Globals): any,
    evaluateClosure(code: Closure, scope: Globals, args: any[]): any
}
export interface Compiler {
    compile(trExpr: any): ByteCode
}
export interface AnimaMeta {
    id: string,
    vm(maxSteps: number): AnimaVM
    compiler(): Compiler
    deepPrint(bc: ByteCode): void;
}

export class Anima {
    #vm: AnimaVM
    #comp: Compiler
    #scope: Globals
    #impl: AnimaMeta
    #t = new AnimaTransformer()

    get scope() {
        return this.#scope
    }

    get compiler() {
        return this.#comp
    }

    get vm() {
        return this.#vm
    }

    constructor(impl: AnimaMeta, maxSteps?: number) {
        this.#impl = impl
        this.#vm = impl.vm(maxSteps || 0)
        this.#comp = impl.compiler()
        const publicScope = getBootstrapScopeFor(impl, this.#comp, this.#vm, this.#t)
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

    compileAstToClosure(bast: any, args: any[] | DottedPair, globals: Globals): Closure {
        const ast = [OP_LAMBDA, args, bast]
        const bc = this.compileRawAst(ast)
        const res = this.#vm.evaluateRaw(bc, globals) // Use the VM to create the closure
        return res
    }

    compileRaw(s: string) {
        const ast = new ASP(s, true).parse()
        return this.compileRawAst(ast)
    }

    compileRawAst(ast: any) {
        return _compileRawAst(ast, this.#comp, this.#t)
    }

    deepPrint(bc: ByteCode) {
        this.#impl.deepPrint(bc)
    }
}

const _compileRawAst = (ast: any, cmp: Compiler, t: AnimaTransformer) => {
    let trExpr = t.transform(ast)
    return cmp.compile(trExpr)
}

const bootstrappedPreludes: Map<string, Globals> = new Map()
const getBootstrapScopeFor = (impl: AnimaMeta, cmp: Compiler, vm: AnimaVM, t: AnimaTransformer) => {
    if (bootstrappedPreludes.has(impl.id)) {
        return bootstrappedPreludes.get(impl.id)!
    }
    const preludeAst = new ASP(STD_PRELUDE, true).parse()
    const PRELUDE_BC = _compileRawAst(preludeAst, cmp, t)
    //impl.deepPrint(PRELUDE_BC)
    const privScope = stdPreludeScope()
    vm.evaluateRaw(PRELUDE_BC, privScope)

    /* Base scope */
    const publicScope = Globals.newWith({}, true); 
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

    bootstrappedPreludes.set(impl.id, publicScope)
    return publicScope
}