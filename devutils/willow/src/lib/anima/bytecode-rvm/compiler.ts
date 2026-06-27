
import { ASP, ASTStringifier, DottedPair, ensureCanBind, normalizeExpr, OP_ADD, OP_AND, OP_APPLY, OP_BEGIN, OP_COND, OP_DEFINE, OP_DIV, OP_EQ, OP_IF, OP_LAMBDA, OP_LET, OP_LETREC, OP_LETSTAR, OP_LIST, OP_MODULO, OP_MUL, OP_OR, OP_QUOTE, OP_REMAINDER, OP_SET, OP_SUB } from "../common";
import { AnimaTransformer } from "../syntax-transformer";
import { CompilerScope } from "./scope";
import { ClosureTemplate, type UpVarLoc } from "./vm";

export class JumpLabel {
    public id: number;
    constructor() { this.id = Math.random(); } 
}

type JumpCond = "True" | "False"

type Node = {
    t: "LoadValue",
    destReg: number,
    constant: any // will later on become a LOAD__COMPLEX/TRUE/FALSE/EMPTYLIST/VOID/U8
} | {
    t: "Move",
    destReg: number,
    srcReg: number,
} | {
    t: "Negate",
    reg: number,
} | {
    t: "LoadUpvar",
    destReg: number,
    upvarIdx: number
} | {
    t: "SetUpvar",
    srcReg: number,
    upvarIdx: number
} | {
    t: "LoadGlobal",
    destReg: number,
    sym: symbol
} | {
    t: "SetGlobal",
    srcReg: number,
    sym: symbol
} | {
    t: "HasGlobal",
    sym: symbol
} | {
    t: "Label",
    label: JumpLabel
} | {
    t: "CondJump", // internally specializes into JUMPIFTRUE or JUMPIFFALSE instructions later on during emission
    reg: number,
    label: JumpLabel,
    cond: JumpCond
} | {
    t: "Jump",
    label: JumpLabel
} | {
    t: "Call",
    procReg: number,
    destReg: number, // ret value is stored in destReg
    startReg: number,
    nargs: number,
} | {
    t: "TailCall",
    procReg: number,
    // does not return to caller so no ret value needed
    startReg: number,
    nargs: number,
} | {
    t: "Return",
    reg: number
} | {
    t: "NewClosure",
    destReg: number,
    template: ClosureTemplateIR
} | {
    // Last arg must be a list
    t: "IApply",
    procReg: number,
    destReg: number,
    startReg: number,
    nargs: number,
} | {
    // Last arg must be a list
    t: "ITailApply",
    procReg: number,
    startReg: number,
    nargs: number,
} | {
    // Not really a bytecode op, but can be optimized/constant folded (or emitted as IADD/ISUB/IMUL/IDIV/IMODULO/IREMAINDER ops etc.)
    t: "IBuiltin",
    f: BuiltinFunction
    destReg: number,
    startReg: number,
    nargs: number
} | {
    // Not really a bytecode op, but can be optimized out or emitted as IASSERTNUMBER ops etc.
    t: "IBuiltinAssert",
    f: BuiltinFunction
    startReg: number,
    nargs: number
}

/** A template for a closure that can then be bound to a scope */
export class ClosureTemplateIR {
    params: symbol[]; // base (individual param binds)
    remParams: symbol | null; // where the remaining params should be bound too (if any). This implicitly makes a closure variadic as well
    code: Node[]
    numLocals: number;
    upvarLocs: UpVarLoc[] // what upvars do we need to capture

    constructor(params: symbol[], remParams: symbol | null, code: Node[], numLocals: number, upvarLocs: UpVarLoc[]) {
        this.params = params
        this.remParams = remParams
        this.code = code
        this.numLocals = numLocals
        this.upvarLocs = upvarLocs
    }
}

export class BuiltinFunction {
    constructor(
        public name: string,
        public cb: (args: any[]) => any,
        public generate: (compiler: Compiler, destReg: number, argRegs: number[]) => void,
        public assert: (compiler: Compiler, argRegs: number[]) => void,
    ) {}
}

const IBUILTINS: Record<symbol, BuiltinFunction> = {
}

interface CmpOpts {
    destReg?: number // where to store dest reg
    isTail: boolean // whether this is a tail-call or not (for tco)
    nodes: Node[]
    scope: CompilerScope
}

export class Compiler {
    s = new ASTStringifier()
    t = new AnimaTransformer()

    constructor() {}

    compile(s: string) {
        const ast = new ASP(s, true).parse()

        // Step 1 is to apply the syntax transformation of cond/let/let*/letrec into simple form
        let trExpr = this.t.transform(ast)

        const scope = new CompilerScope(null)
        const nodes: Node[] = []
        const retReg = scope.allocTemp(); // no need to free the temp reg as we return?
        this.#compile(trExpr, {destReg: retReg, isTail: true, nodes, scope})
        nodes.push({t: "Return", reg: retReg})
    }

    #compile(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        // Raw values
        if (typeof expr === 'symbol') {
            const res = this.#getVar(expr, opts.scope, opts.destReg)
            if (res) opts.nodes.push(res)
            return
        } else if (expr instanceof DottedPair) {
            throw new Error(`bad syntax: illegal use of dotted pair in execution context (consider quoting e.g. ${`'${this.s.stringify(expr)}`})`);
        } else if (!Array.isArray(expr)) { // non array (string etc.)
            if (opts.destReg === undefined) return  
            opts.nodes.push({t: "LoadValue", constant: expr, destReg: opts.destReg})
            return
        }

        if (expr.length === 0) {
            // An empty array evaluates to null
            if (opts.destReg === undefined) return 
            opts.nodes.push({t: "LoadValue", constant: [], destReg: opts.destReg})
            return
        }

        const operator = expr[0];

        if (typeof operator === "symbol") {
            switch (operator) {
                case OP_BEGIN:
                    this.#compileBegin(expr, opts, syntaxCtx)
                    return
                case OP_IF:
                    this.#compileIfCall(expr, opts, syntaxCtx)
                    return
                case OP_QUOTE:
                    this.#compileQuote(expr, opts, syntaxCtx)
                    return
                case OP_DEFINE:
                    this.#compileDefine(expr, opts, syntaxCtx)
                    return
                case OP_SET:
                    this.#compileSet(expr, opts, syntaxCtx)
                    return
                case OP_LAMBDA:
                    this.#compileLambda(expr, opts, syntaxCtx)
                    return
                case OP_AND:
                    this.#compileAnd(expr, opts, syntaxCtx)
                    return
                case OP_OR:
                    this.#compileOr(expr, opts, syntaxCtx)
                    return
                case OP_APPLY:
                    this.#optApply(expr, opts, syntaxCtx)
                    return
                // Intrinsic optimizations
                case OP_ADD:
                case OP_SUB:
                case OP_MUL:
                case OP_DIV:
                case OP_EQ:
                case OP_MODULO:
                case OP_REMAINDER:
                case OP_LIST:
                    const int = IBUILTINS[operator]
                    if(int) this.#optIntrinsicNormal(expr, int, opts, syntaxCtx)
                    else throw new Error(`internal error: failed to find intrinsic for ${String(operator)}`)
                    return
                case OP_LET:
                case OP_LETSTAR:
                case OP_LETREC:
                case OP_COND:
                    throw new Error("internal error: let/let*/letrec/cond should be transformed by AnimaTransform prior to reaching here")
            }
        }

        this.#compileNormalCall(expr, opts)
    }

    #compileBegin(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // We need to load a void if we see an empty begin block
        if (expr.length === 1) {
            if (opts.destReg === undefined) return 
            opts.nodes.push({t: "LoadValue", constant: undefined, destReg: opts.destReg})
            return
        }

        for (let i = 1; i < expr.length; i++) {
            // the child is a tail call only if we are a tail call and its the last child
            const isLastChild = (i === expr.length - 1);
            const childIsTail = isLastChild && opts.isTail; 
            this.#compile(expr[i], { ...opts, destReg: isLastChild ? opts.destReg : undefined, isTail: childIsTail });
        }
    }

    // compiles both if calls as well as code that is converted into if calls
    #compileIfCall(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if(expr.length != 4) {
            throw new Error(`if condition must be in format ["if", condition, true_expr, false_expr] but only have ${expr.length-1} arguments`)
        }

        // We need to compile the first arg first and leave it to a temp reg
        const condReg = opts.scope.allocTemp()
        this.#compile(expr[1], { ...opts, destReg: condReg, isTail: false })
        // we place the bytecode as <jumpiffalse [false code]><true code><jump [|]><false code>|
        const falseLabel = new JumpLabel()
        const endLabel = new JumpLabel()
        opts.nodes.push({t: "CondJump", cond: "False", label: falseLabel, reg: condReg})
        opts.scope.freeTemp(condReg) // we can free the reg here
        // Place true code
        this.#compile(expr[2], opts)
        // Place jump to end
        opts.nodes.push({t: "Jump", label: endLabel})
        // Place false code as well as jump to start of false code
        opts.nodes.push({t:"Label", label: falseLabel})
        this.#compile(expr[3], opts)
        // Fix jump to end to now jump to after false code
        opts.nodes.push({t: "Label", label: endLabel})
    }

    #compileQuote(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if(expr.length != 2) {
            throw new Error(`quote must be in format ["quote", expr] but have ${expr.length-1} arguments`)
        }
        if(opts.destReg === undefined) return
        opts.nodes.push({t: "LoadValue", constant: normalizeExpr(expr[1]), destReg: opts.destReg})
    }

    // note that the syntax transformer alr handles defines inside a lambda so
    #compileDefine(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if (opts.scope.outer) {
            throw new Error(`internal error: define found inside lambda-expr or other lexical scoping (let/let*/letrec), internal defines should be transformed by AnimaTransform prior to reaching here`);
        }

        if(expr.length !== 3) {
            throw new Error(`define must be in format ["define" varname arg] (post transformation) but have ${expr.length-1} arguments`)
        }

        if(typeof expr[1] !== "symbol") throw new Error("internal error: complex defines should be transformed by AnimaTransform prior to reaching here")

        // By now, everything here should be a normal define
        ensureCanBind(expr[1], undefined, syntaxCtx || "define")

        // We need to compile the second arg first and leave it on a temp reg
        const valReg = opts.scope.allocTemp();
        this.#compile(expr[2], { ...opts, destReg: valReg, isTail: false });
        opts.nodes.push({t: "SetGlobal", srcReg: valReg, sym: expr[1]})
        opts.scope.freeTemp(valReg)
        if (opts.destReg !== undefined) {
            opts.nodes.push({t: "LoadValue", destReg: opts.destReg, constant: undefined})
        }
    }

    #compileSet(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if(expr.length != 3) {
            throw new Error(`set! must be in format ["set!" varname arg] but have ${expr.length-1} arguments`)
        }

        if(typeof expr[1] !== "symbol") {
            throw new Error(`${String(expr[1])}: expr[1] not symbol or list syntax`)
        }

        ensureCanBind(expr[1], undefined, syntaxCtx || "set!")

        // We need to compile the second arg first and leave it on a temp reg
        const valReg = opts.scope.allocTemp();
        this.#compile(expr[2], { ...opts, destReg: valReg, isTail: false });
        const res = this.#setVar(expr[1], opts.scope, valReg)
        if(res) opts.nodes.push(res)
        opts.scope.freeTemp(valReg)
        if (opts.destReg !== undefined) {
            opts.nodes.push({t: "LoadValue", destReg: opts.destReg, constant: undefined})
        }
    }

    #compileLambda(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if(expr.length < 3) {
            throw new Error(`lambda must be in format ["lambda", [bind-args...], body...] but only have ${expr.length-1} arguments`)
        }

        const lambdaScope = new CompilerScope(opts.scope)

        let params: symbol[] = []
        let remParams: symbol | null = null
        if (Array.isArray(expr[1])) {
            params = expr[1]
        } else if (expr[1] instanceof DottedPair) {
            // Bind params to items and remParam to remParams
            params = expr[1].items
            remParams = expr[1].rest
        } else if (typeof expr[1] === "symbol") {
            // Then all args must be bound to remparams
            remParams = expr[1]
        } else {
            throw new Error(`${syntaxCtx || "lambda"} arguments must be a symbol (to bind all as a list to said symbol) or a list`);
        }

        // Validate params and remParams here
        const seen = new Set<symbol>();
        for(let i = 0; i < params.length; i++) {
            ensureCanBind(params[i], seen, syntaxCtx || "lambda")
            lambdaScope.addLocal(params[i])
        }
        if (remParams) {
            ensureCanBind(remParams, seen, syntaxCtx || "lambda")
            lambdaScope.addLocal(remParams)
        }

        // Once we've verified the syntax, we can then drop the entire lambda if its not actually needed
        if (opts.destReg === undefined) return

        // Compile lambda body
        const lambdaNodes: Node[] = []
        const retReg = lambdaScope.allocTemp() // no need to free the temp reg as we return?
        this.#compile(this.#wrapMulti(expr.slice(2)), {...opts, destReg: retReg, isTail: true, nodes: lambdaNodes, scope: lambdaScope })
        
        // Only emit return if we need to
        const lastNode = lambdaNodes.length > 0 ? lambdaNodes[lambdaNodes.length - 1] : null;
        const isTerminal = lastNode && ["TailCall", "ITailApply", "Return"].includes(lastNode.t);
        if (!isTerminal) {
            lambdaNodes.push({t: "Return", reg: retReg})
        }
        const template = new ClosureTemplateIR(params, remParams, lambdaNodes, lambdaScope.numLocals, lambdaScope.upvars);
        opts.nodes.push({t: "NewClosure", template: template, destReg: opts.destReg})
    }

    #compileAnd(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // if (argCount === 0) return true; 
        if (expr.length === 1) {
            if (opts.destReg !== undefined) opts.nodes.push({t: "LoadValue", destReg: opts.destReg, constant: true})
            return;
        }

        const endLabel = new JumpLabel()

        // OPTIMIZATION: Notice that the only falsy value in scheme is #f so the moment we fall through
        // the condJump with a cond of False, the value in the dest reg is *false*
        const targetReg = opts.destReg === undefined ? opts.scope.allocTemp() : opts.destReg!;
        for (let i = 1; i < expr.length - 1; i++) {
            this.#compile(expr[i], { ...opts, destReg: targetReg, isTail: false })
            opts.nodes.push({ t: "CondJump", reg: targetReg, label: endLabel, cond: "False" })
        }

        // tail expr is the last cond so it gets directly evaluated (if all the and condjumps get through)
        // This inherits the parent's destReg and isTail state so we get free tailcall + value stored in right dest reg
        this.#compile(expr[expr.length - 1], opts)
        if (opts.nodes[opts.nodes.length - 1].t !== "TailCall" && opts.nodes[opts.nodes.length - 1].t !== "ITailApply") {
            opts.nodes.push({ t: "Jump", label: endLabel })
        }
        opts.nodes.push({ t: "Label", label: endLabel });
        if (opts.destReg === undefined) opts.scope.freeTemp(targetReg);
    }

    #compileOr(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // if (argCount === 0) return false; 
        if (expr.length === 1) {
            if (opts.destReg !== undefined) opts.nodes.push({t: "LoadValue", destReg: opts.destReg, constant: false})
            return;
        }

        const endLabel = new JumpLabel()

        // OPTIMIZATION: Notice that scheme needs the or to short circuit with the last eval'd value soooo
        const targetReg = opts.destReg === undefined ? opts.scope.allocTemp() : opts.destReg!;
        for (let i = 1; i < expr.length - 1; i++) {
            this.#compile(expr[i], { ...opts, destReg: targetReg, isTail: false })
            opts.nodes.push({ t: "CondJump", reg: targetReg, label: endLabel, cond: "True" })
        }

        // tail expr is the last cond so it gets directly evaluated (if all the or condjumps get through)
        // This inherits the parent's destReg and isTail state so we get free tailcall + value stored in right dest reg
        this.#compile(expr[expr.length - 1], opts)
        if (opts.nodes[opts.nodes.length - 1].t !== "TailCall" && opts.nodes[opts.nodes.length - 1].t !== "ITailApply") {
            opts.nodes.push({ t: "Jump", label: endLabel })
        }
        opts.nodes.push({ t: "Label", label: endLabel });
        if (opts.destReg === undefined) opts.scope.freeTemp(targetReg);
    }

    // a normal call
    // TODO: make this slightly faster
    #compileNormalCall(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // Try IIFE optimizations
        if(this.#optIIFE(expr, opts)) {
            return
        }

        // We need to compile the proc and place it on its own tempval
        const procReg = opts.scope.allocTemp();
        this.#compile(expr[0], { ...opts, destReg: procReg, isTail: false })

        // Push all arguments to a contiguous reg block
        const nargs = expr.length-1 // [func a b c] -> 5 - 2 = 3 args
        const startReg = opts.scope.regAlloc.allocBlock(nargs);
        for (let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, destReg: startReg+i-1, isTail: false })
        }

        if (opts.isTail) {
            opts.nodes.push({t: "TailCall", nargs, procReg, startReg})
        } else {
            const targetReg = opts.destReg === undefined ? opts.scope.allocTemp() : opts.destReg!;
            opts.nodes.push({t: "Call", destReg: targetReg, nargs, procReg, startReg})
            if (opts.destReg === undefined) opts.scope.freeTemp(targetReg);
        }

        opts.scope.regAlloc.freeBlock(startReg, nargs)
        opts.scope.freeTemp(procReg)
    }

    #optIIFE(expr: any[], opts: CmpOpts, syntaxCtx?: string): boolean {
        // if we have a non-variadic IIFE ((lambda (params...) body) args...), then we can optimize it down
        // to BLOCK/ENDBLOCK instead of doing a whole function call
        //
        // TODO: add a thing to transformer etc to track down call/cc calls so we can bail out if we see a call/cc
        // call. Not important right now though as we don't support call/cc
        const first = expr[0]
        if (Array.isArray(first) && first[0] === OP_LAMBDA && Array.isArray(first[1])) {
            const params = first[1];
            const body = first.slice(2);
            const args = expr.slice(1)
            if (params.length !== args.length) {
                throw new Error(`expected exactly ${params.length} args, got ${args.length}`);
            }
            
            // Bind all arguments in prior to entering the iife's block
            const seen = new Set<symbol>();
            const regs: number[] = []
            for(let i = 0; i < params.length; i++) {
                ensureCanBind(params[i], seen, syntaxCtx || "lambda")

                // The vm guarantees that locals are initially undefined as a default value
                if (args[i] === undefined) {
                    continue
                }
                const destReg = opts.scope.allocTemp()
                this.#compile(args[i], { ...opts, destReg: destReg, isTail: false })
                regs.push(destReg)
            }

            opts.scope.enterBlock()
            // alloc slots for every parameter
            for(let i = 0; i < params.length; i++) {
                opts.scope.addLocal(params[i])
            }
            // setlocal all of our args in reverse order
            for(let i = params.length - 1; i >= 0; i--) {
                if (args[i] === undefined) {
                    continue 
                }
                const res = this.#setVar(params[i], opts.scope, regs[i]) // note to self: setVar calls resolve internally which will then yield the exact slot
                if(res) opts.nodes.push(res)
            }

            this.#compile(this.#wrapMulti(body), opts)
            opts.scope.exitBlock()

            for(const reg of regs) opts.scope.freeTemp(reg)
            return true
        } else {
            return false
        }
    }

    /** Optimizes a direct (apply proc elems... rem-arg-lst) to inline INTRINSIC_APPLY */ 
    #optApply(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if (expr.length < 3) {
            throw new Error("apply requires at least a procedure and a list");
        }

        // Push proc
        const procReg = opts.scope.allocTemp();
        this.#compile(expr[1], { ...opts, destReg: procReg, isTail: false });
        // Push args
        const nargs = expr.length - 2; // expr - Symbol(apply) - proc
        const startReg = opts.scope.regAlloc.allocBlock(nargs);
        for(let i = 2; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, destReg: startReg+i-2, isTail: false });
        }
        // Now we're ready to do a INTRINSIC_APPLY 
        if (opts.isTail) {
            opts.nodes.push({t: "ITailApply", nargs, procReg, startReg})
        } else {
            const targetReg = opts.destReg === undefined ? opts.scope.allocTemp() : opts.destReg!;
            opts.nodes.push({t: "IApply", destReg: targetReg, nargs, procReg, startReg})
            if (opts.destReg === undefined) opts.scope.freeTemp(targetReg);
        }

        opts.scope.regAlloc.freeBlock(startReg, nargs)
        opts.scope.freeTemp(procReg)
    }

    /** Optimizes intrinsic ops to a INTRINSIC_ADD/SUB/MUL/DIV */ 
    #optIntrinsicNormal(expr: any[], int: BuiltinFunction, opts: CmpOpts, syntaxCtx?: string) {
        // Push args
        const nargs = expr.length - 1; // expr - Symbol(op)
        const startReg = opts.scope.regAlloc.allocBlock(nargs);
        for(let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, destReg: startReg + (i - 1), isTail: false });
        }

        if (opts.destReg !== undefined) {
            opts.nodes.push({t: "IBuiltin", destReg: opts.destReg, startReg, nargs, f: int})
        } else {
            opts.nodes.push({t: "IBuiltinAssert", startReg, nargs, f: int})
        }

        opts.scope.regAlloc.freeBlock(startReg, nargs)
    }

    #wrapMulti = (exprs: any[]) => {
        if (exprs.length === 0) return []; 
        if (exprs.length === 1) return exprs[0];
        return [OP_BEGIN, ...exprs];
    }

    #getVar(varname: symbol, scope: CompilerScope, destReg?: number): Node | null {
        // Check if we can resolve it to a local/upvar
        const resolved = scope.resolve(varname)

        if (resolved.type === 'Local') {
            // If we need to emit a move, then move, otherwise do nothing
            if (destReg !== undefined && resolved.index != destReg) {
                return {t: "Move", srcReg: resolved.index, destReg }
            }
            return null
        } 
        
        if (resolved.type === 'Upvar') {
            if (destReg !== undefined) {
                return {t: "LoadUpvar", upvarIdx: resolved.index, destReg }
            }
            return null
        }

        // Assume global
        if (destReg === undefined) return {t: "HasGlobal", sym: varname}
        return {t: "LoadGlobal", sym: varname, destReg}
    }

    #setVar(varname: symbol, scope: CompilerScope, srcReg: number): Node | null {
        // Check if we can resolve it to a local/upvar
        const resolved = scope.resolve(varname)

        if (resolved.type === 'Local') {
            if (srcReg === resolved.index) return null;
            return { t: "Move", srcReg, destReg: resolved.index }
        } 
        
        if (resolved.type === 'Upvar') {
            return { t: "SetUpvar", srcReg, upvarIdx: resolved.index }
        }

        // Assume global
        return {t: "SetGlobal", sym: varname, srcReg}
    }
}