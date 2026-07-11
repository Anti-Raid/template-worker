import { ASTStringifier, DottedPair, ensureCanBind, normalizeExpr, OP_AND, OP_BEGIN, OP_COND, OP_DEFINE, OP_IF, OP_LAMBDA, OP_LET, OP_LETREC, OP_LETSTAR, OP_OR, OP_QUOTE, OP_SET, unpackLambdaExprArgs, wrapMulti } from "../common"
import { IBUILTINS_IDX_MAP } from "../std"
import { AnimaTransformer } from "../syntax-transformer"
import { IR, type Node, JumpLabel, ClosureTemplateIR } from "./ir"
import { CompilerScope } from "./scope"

interface CmpOpts {
    leaveOnStack: boolean // whether to leave created values on the stack or not
    isTail: boolean // whether this is a tail-call or not (for tco)
    nodes: Node[]
    scope: CompilerScope
}

export class Compiler {
    #s = new ASTStringifier()
    #t = new AnimaTransformer()

    constructor() {}

    compileRawAst(ast: any) {
        // We need to transform all the special syntax down first
        let trExpr = this.#t.transform(ast)

        const scope = new CompilerScope(null)
        const nodes: Node[] = []
        this.#compile(trExpr, {leaveOnStack: true, isTail: true, nodes, scope})
        if (!this.#nodesEndsInRet(nodes)) {
            nodes.push({t: "Return"})
        }
        const ir = new IR()
        return ir.lower(nodes, scope.numLocals)
    }

    #compile(expr: any, opts: CmpOpts) {
        // Raw values
        if (typeof expr === 'symbol') {
            const res = this.#getVar(expr, opts)
            if (res) opts.nodes.push(res)
            return
        } else if (expr instanceof DottedPair) {
            throw new Error(`bad syntax: illegal use of dotted pair in execution context (consider quoting e.g. ${`'${this.#s.stringify(expr)}`})`);
        } else if (!Array.isArray(expr)) { // non array (string etc.)
            if (!opts.leaveOnStack) return  
            opts.nodes.push({t: "PushValue", constant: expr})
            return
        }

        if (expr.length === 0) {
            // An empty array evaluates to null
            if (!opts.leaveOnStack) return
            opts.nodes.push({t: "PushValue", constant: []})
            return
        }

        const operator = expr[0];

        if (typeof operator === "symbol") {
            switch (operator) {
                case OP_BEGIN:
                    this.#compileBegin(expr, opts)
                    return
                case OP_IF:
                    this.#compileIfCall(expr, opts)
                    return
                case OP_QUOTE:
                    this.#compileQuote(expr, opts)
                    return
                case OP_DEFINE:
                    this.#compileDefine(expr, opts)
                    return
                case OP_SET:
                    this.#compileSet(expr, opts)
                    return
                case OP_LAMBDA:
                    this.#compileLambda(expr, opts)
                    return
                case OP_AND:
                    this.#compileAnd(expr, opts)
                    return
                case OP_OR:
                    this.#compileOr(expr, opts)
                    return
                case OP_LET:
                case OP_LETSTAR:
                case OP_LETREC:
                case OP_COND:
                    throw new Error("internal error: let/let*/letrec/cond should be transformed by AnimaTransform prior to reaching here")
            }
        }

        // intrinsic
        const builtinsIdx = IBUILTINS_IDX_MAP.get(operator)
        if (builtinsIdx !== undefined) {
            this.#optIntrinsicNormal(expr, builtinsIdx, opts)
            return
        }

        this.#compileNormalCall(expr, opts)
    }

    #compileBegin(expr: any[], opts: CmpOpts) {
        // We need to push a void if we see an empty begin block
        if (expr.length === 1) {
            if (opts.leaveOnStack) opts.nodes.push({t: "PushValue", constant: undefined});
            return;
        }

        for (let i = 1; i < expr.length; i++) {
            // the child is a tail call only if we are a tail call and its the last child
            const isLastChild = (i === expr.length - 1);
            const childIsTail = isLastChild && opts.isTail; 
            this.#compile(expr[i], { ...opts, leaveOnStack: isLastChild, isTail: childIsTail });
        }
    }

    // compiles both if calls as well as code that is converted into if calls
    #compileIfCall(expr: any[], opts: CmpOpts) {
        if(expr.length != 4) {
            throw new Error(`if condition must be in format ["if", condition, true_expr, false_expr] but only have ${expr.length-1} arguments`)
        }

        const falseLabel = new JumpLabel()
        const endLabel = new JumpLabel()

        // We need to compile the first arg first and leave it on the stack
        this.#compile(expr[1], { ...opts, leaveOnStack: true, isTail: false })
        // we place the bytecode as <jumpiffalse [false code]><true code><jump [|]><false code>|
        opts.nodes.push({t: "CondJump", cond: "False", label: falseLabel})
        // Place true code
        this.#compile(expr[2], opts)
        // Place jump to end
        if (!this.#nodesEndsInRet(opts.nodes)) {
            opts.nodes.push({t: "Jump", label: endLabel})
        }
        // Place false code as well as jump to start of false code
        opts.nodes.push({t:"Label", label: falseLabel})
        this.#compile(expr[3], opts)
        // Fix jump to end to now jump to after false code
        opts.nodes.push({t: "Label", label: endLabel})
    }

    #compileQuote(expr: any[], opts: CmpOpts) {
        if(expr.length != 2) {
            throw new Error(`quote must be in format ["quote", expr] but have ${expr.length-1} arguments`)
        }
        if(!opts.leaveOnStack) return
        opts.nodes.push({t: "PushValue", constant: normalizeExpr(expr[1])})
    }

    // note that the syntax transformer alr handles defines inside a lambda so
    #compileDefine(expr: any[], opts: CmpOpts) {
        if(typeof expr[1] !== "symbol") throw new Error("internal error: complex defines should be transformed by AnimaTransform prior to reaching here")
        // AnimaTransform ensures defines are of correct form
        //
        // We need to compile the second arg first and leave it on the stack
        this.#compile(expr[2], { ...opts, leaveOnStack: true, isTail: false });
        opts.nodes.push({t: "SetGlobal", sym: expr[1]})
        if (opts.leaveOnStack) {
            opts.nodes.push({t: "PushValue", constant: undefined})
        }
    }

    #compileSet(expr: any[], opts: CmpOpts) {
        // AnimaTransform ensures sets are of correct form
        //
        // We need to compile the second arg first and leave it on the stack
        this.#compile(expr[2], { ...opts, leaveOnStack: true, isTail: false });
        opts.nodes.push(this.#setVar(expr[1], opts))
        if (opts.leaveOnStack) {
            opts.nodes.push({t: "PushValue", constant: undefined})
        }
    }

    #compileAnd(expr: any[], opts: CmpOpts) {
        if (expr.length === 1) {
            if (opts.leaveOnStack) opts.nodes.push({t: "PushValue", constant: true})
            return;
        }

        const endLabel = new JumpLabel()

        for (let i = 1; i < expr.length - 1; i++) {
            this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false })
            if (opts.leaveOnStack) {
                // If parent wants return value, we cannot just JIF as jump will pop ret val
                //
                // Instead, DUP top of stack, so JIF pops the duplicate leaving original value safely
                // on top of stack (short-circuit). If the jump falls through, we pop out that value 
                // and move on
                opts.nodes.push({ t: "Dup" });
                opts.nodes.push({ t: "CondJump", cond: "False", label: endLabel });
                opts.nodes.push({ t: "Pop" });
            } else {
                opts.nodes.push({ t: "CondJump", label: endLabel, cond: "False" })
            }
        }

        // tail expr is the last cond so it gets directly evaluated (if all the and condjumps get through)
        // This inherits the parents isTail+leaveOnStack state so we get free tailcall
        this.#compile(expr[expr.length - 1], opts)
        opts.nodes.push({ t: "Label", label: endLabel });
    }

    #compileOr(expr: any[], opts: CmpOpts) {
        if (expr.length === 1) {
            if (opts.leaveOnStack) opts.nodes.push({t: "PushValue", constant: false})
            return;
        }

        const endLabel = new JumpLabel()

        for (let i = 1; i < expr.length - 1; i++) {
            this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false })
            if (opts.leaveOnStack) {
                // If parent wants return value, we cannot just JIT as jump will pop ret val
                //
                // Instead, DUP top of stack, so JIT pops the duplicate leaving original value safely
                // on top of stack (short-circuit). If the jump falls through, we pop out that value 
                // and move on
                opts.nodes.push({ t: "Dup" });
                opts.nodes.push({ t: "CondJump", cond: "True", label: endLabel });
                opts.nodes.push({ t: "Pop" });
            } else {
                opts.nodes.push({ t: "CondJump", label: endLabel, cond: "True" })
            }
        }

        // tail expr is the last cond so it gets directly evaluated (if all the and condjumps get through)
        // This inherits the parents isTail+leaveOnStack state so we get free tailcall
        this.#compile(expr[expr.length - 1], opts)
        opts.nodes.push({ t: "Label", label: endLabel });
    }

    #compileLambda(expr: any[], opts: CmpOpts) {
        // AnimaTransform ensures lambdas are of correct form
        const { params, remParams } = unpackLambdaExprArgs(expr, IBUILTINS_IDX_MAP, "lambda")

        // Once we've verified the syntax, we can then drop the entire lambda if its not actually needed
        if (!opts.leaveOnStack) return

        const lambdaScope = new CompilerScope(opts.scope)
        const lambdaNodes: Node[] = []
        for(let i = 0; i < params.length; i++) {
            lambdaScope.addLocal(params[i])
        }
        if (remParams) {
            lambdaScope.addLocal(remParams)
        }

        // Compile lambda body
        this.#compile(wrapMulti(expr.slice(2)), {...opts, leaveOnStack: true, isTail: true, nodes: lambdaNodes, scope: lambdaScope })
        if (!this.#nodesEndsInRet(lambdaNodes)) {
            lambdaNodes.push({t: "Return"})
        }
        const template = new ClosureTemplateIR(params, remParams, lambdaNodes, lambdaScope.numLocals, lambdaScope.upvars);
        opts.nodes.push({t: "NewClosure", template: template})
    }

    // a normal call
    #compileNormalCall(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // Try IIFE optimizations
        if(this.#optIIFE(expr, opts)) {
            return
        }

        // Push func
        this.#compile(expr[0], { ...opts, leaveOnStack: true, isTail: false })
        // Push args
        const nargs = expr.length - 1; // expr - Symbol(op)
        for(let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false });
        }

        if (opts.isTail) {
            opts.nodes.push({t: "TailCall", nargs})
        } else {
            opts.nodes.push({t: "Call", nargs})
            if (!opts.leaveOnStack) opts.nodes.push({t: "Pop"}) // Popping only matters in non-tail-calls
        }
    }

    #optIIFE(expr: any[], opts: CmpOpts): boolean {
        // if we have a non-variadic IIFE ((lambda (params...) body) args...), then we can optimize it down
        // to BLOCK/ENDBLOCK instead of doing a whole function call
        const first = expr[0]
        if (Array.isArray(first) && (first[0] === OP_LAMBDA) && Array.isArray(first[1])) {
            //console.log("Applying IIFE")
            const params = first[1];
            const body = first.slice(2);
            const args = expr.slice(1)
            if (params.length !== args.length) {
                throw new Error(`expected exactly ${params.length} args, got ${args.length}`);
            }
            
            // Bind all arguments outside new block scope
            const seen = new Set<symbol>();
            for(let i = 0; i < args.length; i++) {
                ensureCanBind(params[i], seen, "lambda", IBUILTINS_IDX_MAP)
                if (args[i] === undefined) continue // the vm guarantees that locals are initially undefined as a default value
                this.#compile(args[i], { ...opts, leaveOnStack: true, isTail: false });
            }

            // Now enter block and alloc slots
            opts.scope.enterBlock()
            for(let i = 0; i < params.length; i++) {
                opts.scope.addLocal(params[i])            
            }

            // setlocal all of our args in reverse order
            for(let i = params.length - 1; i >= 0; i--) {
                if (args[i] === undefined) {
                    continue 
                }
                opts.nodes.push(this.#setVar(params[i], opts)) // note to self: setVar calls resolve internally which will then yield the exact slot
            }

            this.#compile(wrapMulti(body), opts)
            opts.scope.exitBlock()
            return true
        } else {
            return false
        }
    }

    /** Optimizes intrinsic/builtin ops */ 
    #optIntrinsicNormal(expr: any[], builtinIdx: number, opts: CmpOpts) {
        // Push func
        opts.nodes.push({t: "PushBuiltin", builtinIdx})
        // Push args
        const nargs = expr.length - 1; // expr - Symbol(op)
        for(let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false });
        }

        if (opts.isTail) {
            opts.nodes.push({t: "TailCall", nargs})
        } else {
            opts.nodes.push({t: "Call", nargs})
            if (!opts.leaveOnStack) opts.nodes.push({t: "Pop"}) // Popping only matters in non-tail-calls
        }
    }

    #nodesEndsInRet(nodes: Node[]) {
        if (nodes.length === 0) return false // we need a return if nodes.length === 0
        const lastNode = nodes[nodes.length-1]
        if (lastNode.t === "TailCall" || lastNode.t === "Return") {
            return true // all of these ops alr return
        }
        return false
    }

    #getVar(varname: symbol, opts: CmpOpts): Node | undefined {
        // Check if we can resolve it to a local/upvar
        const resolved = opts.scope.resolve(varname)

        if (resolved.type === 'Local') {
            if (!opts.leaveOnStack) return undefined // no-op
            return {t: "PushLocal", slot: resolved.index}
        } 
        
        if (resolved.type === 'Upvar') {
            if (!opts.leaveOnStack) return undefined // no-op
            return {t: "PushUpvar", upvarIdx: resolved.index}
        }

        // Assume global
        if (!opts.leaveOnStack) return {t: "HasGlobal", sym: varname} // assert that the global exists as thats the only side-effect
        return {t: "PushGlobal", sym: varname}
    }

    #setVar(varname: symbol, opts: CmpOpts): Node {
        // Check if we can resolve it to a local/upvar
        const resolved = opts.scope.resolve(varname)

        if (resolved.type === 'Local') {
            return {t: "SetLocal", slot: resolved.index}
        } 
        
        if (resolved.type === 'Upvar') {
            return {t: "SetUpvar", upvarIdx: resolved.index}
        }

        // Assume global
        return {t: "SetGlobal", sym: varname}
    }
}