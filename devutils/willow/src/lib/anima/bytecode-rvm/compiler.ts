import { ASP, ASTStringifier, DottedPair, ensureCanBind, normalizeExpr, OP_AND, OP_APPLY, OP_BEGIN, OP_COND, OP_DEFINE, OP_IF, OP_LAMBDA, OP_LET, OP_LETREC, OP_LETSTAR, OP_OR, OP_QUOTE, OP_SET, unpackLambdaExprArgs, wrapMulti } from "../common";
import { AnimaTransformer } from "../syntax-transformer";
import { AstAnalysis } from "./analysis";
import { AnalysisScope, CompilerScope } from "./scope";
import { IR, type Node, JumpLabel, ClosureTemplateIR } from "./ir"
import { IBUILTINS_IDX_MAP } from "../std";

interface CmpOpts {
    destReg?: number // where to store dest reg
    isTail: boolean // whether this is a tail-call or not (for tco)
    nodes: Node[]
    scope: CompilerScope,

    // From pass 1
    ascope: AnalysisScope,
    analyzer: AstAnalysis
}

export class Compiler {
    #s = new ASTStringifier()
    #t = new AnimaTransformer()

    constructor() {}

    compileRaw(s: string) {
        const ast = new ASP(s, true).parse()
        return this.compileRawAst(ast)
    }

    compileRawAst(ast: any) {
        // Step 1 is to apply the syntax transformation of cond/let/let*/letrec into simple form
        let trExpr = this.#t.transform(ast)
        // Step 2 is to analyze our variables so we know what to box and what not to box
        let analyzer = new AstAnalysis()
        const ascope = analyzer.analyze(trExpr)

        const scope = new CompilerScope(null)
        const nodes: Node[] = []
        const retReg = scope.allocTemp(); // no need to free the temp reg as we return?
        this.#compile(trExpr, {destReg: retReg, isTail: true, nodes, scope, ascope, analyzer})
        if (!this.#nodesEndsInRet(nodes)) {
            nodes.push({t: "Return", reg: retReg})
        }
        const ir = new IR()
        return ir.lower(nodes, scope.numRegs)
    }

    #compile(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        // Raw values
        if (typeof expr === 'symbol') {
            const res = this.#getVar(expr, opts, opts.destReg)
            if (res) opts.nodes.push(...res)
            return
        } else if (expr instanceof DottedPair) {
            throw new Error(`bad syntax: illegal use of dotted pair in execution context (consider quoting e.g. ${`'${this.#s.stringify(expr)}`})`);
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
                    this.#optApply(expr, opts)
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
            this.#optIntrinsicNormal(expr, builtinsIdx, opts, syntaxCtx)
            return
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
        if (!this.#nodesEndsInRet(opts.nodes)) {
            opts.nodes.push({t: "Jump", label: endLabel})
        }
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
            throw new Error(`define must be in format ["define" varname arg] (post transformation) but have ${expr.length-1} arguments, complex defines should be transformed by AnimaTransform prior to reaching here`)
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
        // AnimaTransform ensures sets are of correct form

        // We need to compile the second arg first and leave it on a temp reg
        const valReg = opts.scope.allocTemp();
        this.#compile(expr[2], { ...opts, destReg: valReg, isTail: false });
        const res = this.#setVar(expr[1], opts, valReg)
        if(res) opts.nodes.push(...res)
        opts.scope.freeTemp(valReg)
        if (opts.destReg !== undefined) {
            opts.nodes.push({t: "LoadValue", destReg: opts.destReg, constant: undefined})
        }
    }

    #compileLambda(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // AnimaTransform ensures lambdas are of correct form
        const lambdaScope = new CompilerScope(opts.scope)
        const ascope = opts.analyzer.scopeMap.get(expr)
        if (!ascope) throw new Error(`internal error: could not find ascope for expr ${expr}`)
        ascope.dbgPrint()

        const { params, remParams } = unpackLambdaExprArgs(expr, syntaxCtx)
        const lambdaNodes: Node[] = []

        for(let i = 0; i < params.length; i++) {
            const reg = lambdaScope.addLocal(params[i])

            const inf = ascope.getVarinfo(params[i])
            if(!inf) throw new Error("Could not fetch varinfo")
            if(inf.isBoxed) lambdaNodes.push({t: "Box", destReg: reg, srcReg: reg})
        }
        if (remParams) {
            const reg = lambdaScope.addLocal(remParams)

            const inf = ascope.getVarinfo(remParams)
            if(!inf) throw new Error("Could not fetch varinfo")
            if(inf.isBoxed) lambdaNodes.push({t: "Box", destReg: reg, srcReg: reg})
        }

        // Once we've verified the syntax, we can then drop the entire lambda if its not actually needed
        if (opts.destReg === undefined) return

        // Compile lambda body
        const retReg = lambdaScope.allocTemp() // no need to free the temp reg as we return?
        this.#compile(wrapMulti(expr.slice(2)), {...opts, destReg: retReg, isTail: true, nodes: lambdaNodes, scope: lambdaScope, ascope })
        if (!this.#nodesEndsInRet(lambdaNodes)) {
            lambdaNodes.push({t: "Return", reg: retReg})
        }
        const template = new ClosureTemplateIR(params, remParams, lambdaNodes, lambdaScope.numRegs, lambdaScope.upvars);
        opts.nodes.push({t: "NewClosure", template: template, destReg: opts.destReg})
    }

    #nodesEndsInRet(nodes: Node[]) {
        if (nodes.length === 0) return false // we need a return if nodes.length === 0
        const lastNode = nodes[nodes.length-1]
        if (lastNode.t === "TailCall" || lastNode.t === "ApplyTailCall" || lastNode.t === "IBuiltinTail" || lastNode.t === "Return") {
            return true // all of these ops alr return
        }
        return false
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
        opts.nodes.push({ t: "Label", label: endLabel });
        if (opts.destReg === undefined) opts.scope.freeTemp(targetReg);
    }

    // a normal call
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

    // apply blocks can be optimized to either APPLYCALL or APPLYTAILCALL
    #optApply(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // Push all arguments to a contiguous reg block
        const nargs = expr.length-1 // [apply a b c]
        const startReg = opts.scope.regAlloc.allocBlock(nargs);
        for (let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, destReg: startReg+i-1, isTail: false })
        }

        if (opts.isTail) {
            opts.nodes.push({t: "ApplyTailCall", nargs, startReg})
        } else {
            const targetReg = opts.destReg === undefined ? opts.scope.allocTemp() : opts.destReg!;
            opts.nodes.push({t: "ApplyCall", destReg: targetReg, nargs, startReg})
            if (opts.destReg === undefined) opts.scope.freeTemp(targetReg);
        }

        opts.scope.regAlloc.freeBlock(startReg, nargs)
    }

    #optIIFE(expr: any[], opts: CmpOpts, syntaxCtx?: string): boolean {
        // if we have a non-variadic IIFE ((lambda (params...) body) args...), then we can optimize it down
        // to BLOCK/ENDBLOCK instead of doing a whole function call
        const first = expr[0]
        if (Array.isArray(first) && (first[0] === OP_LAMBDA) && Array.isArray(first[1])) {
            console.log("Applying IIFE")
            const ascope = opts.analyzer.scopeMap.get(first)
            if (!ascope) throw new Error(`internal error: could not find ascope for expr ${first}`)

            const params = first[1];
            const body = first.slice(2);
            const args = expr.slice(1)
            if (params.length !== args.length) {
                throw new Error(`expected exactly ${params.length} args, got ${args.length}`);
            }
            
            // Bind all arguments outside new block scope
            const argRegs: number[] = [];
            for(let i = 0; i < args.length; i++) {
                const tempReg = opts.scope.allocTemp();
                this.#compile(args[i], { ...opts, destReg: tempReg, isTail: false });
                argRegs.push(tempReg);
            }

            // Now enter block
            opts.scope.enterBlock()
            const seen = new Set<symbol>();
            for(let i = 0; i < params.length; i++) {
                ensureCanBind(params[i], seen, syntaxCtx || "lambda")
                const inf = ascope.getVarinfo(params[i]);
                if(!inf) throw new Error("Could not fetch varinfo")
                
                const destReg = opts.scope.addLocal(params[i])            
                if(inf.isBoxed) {
                    opts.nodes.push({ t: "Box", srcReg: argRegs[i], destReg });
                } else {
                    opts.nodes.push({ t: "Move", srcReg: argRegs[i], destReg });
                }
            }

            this.#compile(wrapMulti(body), {...opts, ascope})
            opts.scope.exitBlock()
            for (const reg of argRegs) {
                opts.scope.freeTemp(reg);
            }
            return true
        } else {
            return false
        }
    }

    /** Optimizes intrinsic ops to a INTRINSIC_ADD/SUB/MUL/DIV */ 
    #optIntrinsicNormal(expr: any[], builtinIdx: number, opts: CmpOpts, syntaxCtx?: string) {
        // Push args
        const nargs = expr.length - 1; // expr - Symbol(op)
        const startReg = opts.scope.regAlloc.allocBlock(nargs);
        for(let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, destReg: startReg + (i - 1), isTail: false });
        }

        if (opts.isTail) {
            opts.nodes.push({t: "IBuiltinTail", startReg, nargs, builtinIdx})
        } else {
            const targetReg = opts.destReg === undefined ? opts.scope.allocTemp() : opts.destReg!;
            opts.nodes.push({t: "IBuiltin", destReg: targetReg, startReg, nargs, builtinIdx})
            if (opts.destReg === undefined) opts.scope.freeTemp(targetReg);
        }

        opts.scope.regAlloc.freeBlock(startReg, nargs)
    }

    #getVar(varname: symbol, opts: CmpOpts, destReg?: number): Node[] {
        // Check if we can resolve it to a local/upvar
        const resolved = opts.scope.resolve(varname)
        //console.log(resolved)

        if (resolved.type === 'Local') {
            const aresolved = opts.ascope.getVarinfo(varname)
            if (!aresolved) throw new Error(`internal error: ${String(varname)} has no analysis info present`)
            
            if (aresolved.isBoxed) {
                if (destReg !== undefined) {
                    // Unbox
                    return [{t: "Unbox", srcReg: resolved.index, destReg }]
                }
            } else {
                // Move
                if (destReg !== undefined && resolved.index !== destReg) {
                    return [{t: "Move", srcReg: resolved.index, destReg }]
                }
            }
            return []
        } 
        
        if (resolved.type === 'Upvar') {
            const aresolved = opts.ascope.getVarinfo(varname)
            if (!aresolved) throw new Error(`internal error: ${String(varname)} has no analysis info present`)

            if (destReg !== undefined) {
                // right now, we need to load the upvalue in and unbox it (if boxed)
                return [{t: "LoadUpvar", upvarIdx: resolved.index, destReg, andUnbox: aresolved.isBoxed }]
            }
            return []
        }

        // Assume global
        if (destReg === undefined) return [{t: "HasGlobal", sym: varname}]
        return [{t: "LoadGlobal", sym: varname, destReg}]
    }

    #setVar(varname: symbol, opts: CmpOpts, srcReg: number): Node[] {
        // Check if we can resolve it to a local/upvar
        const resolved = opts.scope.resolve(varname)

        if (resolved.type === 'Local') {
            const aresolved = opts.ascope.getVarinfo(varname)
            if (!aresolved) throw new Error(`internal error: ${String(varname)} has no analysis info present`)

            if (aresolved.isBoxed) {
                return [{ t: "SetBox", srcReg, destReg: resolved.index }]
            } else {
                if (srcReg !== resolved.index) {
                    return [{ t: "Move", srcReg, destReg: resolved.index }];
                }
            }
        } 
        
        if (resolved.type === 'Upvar') {
            const aresolved = opts.ascope.getVarinfo(varname)
            if (!aresolved) throw new Error(`internal error: ${String(varname)} has no analysis info present`)

            if (aresolved.isBoxed) {
                const tmpReg = opts.scope.allocTemp()
                const nodes: Node[] = [{t: "LoadUpvar", andUnbox: false, destReg: tmpReg, upvarIdx: resolved.index}, { t: "SetBox", destReg: tmpReg, srcReg }]
                opts.scope.freeTemp(tmpReg)
                return nodes
            } else {
                return [{ t: "SetUpvar", srcReg, upvarIdx: resolved.index, andBox: false }];
            }
        }

        // Assume global
        return [{t: "SetGlobal", sym: varname, srcReg}]
    }
}
