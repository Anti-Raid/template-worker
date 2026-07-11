import { ASTStringifier, DottedPair, ensureCanBind, normalizeExpr, OP_AND, OP_BEGIN, OP_COND, OP_DEFINE, OP_IF, OP_LAMBDA, OP_LET, OP_LETREC, OP_LETSTAR, OP_OR, OP_QUOTE, OP_SET, unpackLambdaExprArgs, wrapMulti } from "../common";
import { AstAnalysis, ContifyAnalyzer } from "./analysis";
import { AnalysisScope, CompilerScope } from "./scope";
import { BUILTINS_START, OP_CONT, OP_CONT_BASECONT } from "./vm";
import { IR, type Node, JumpLabel, ClosureTemplateIR } from "./ir"
import { AstCps } from "./ext-transform";
import { IBUILTINS_IDX_MAP } from "../std";

interface CmpOpts {
    destReg?: number // where to store dest reg
    nodes: Node[]
    scope: CompilerScope,

    // From pass 1
    ascope: AnalysisScope,
    analyzer: AstAnalysis
}

export class Compiler {
    #s = new ASTStringifier()

    constructor() {}

    compile(trExpr_: any) {
        let trExpr = new AstCps().transform(trExpr_) // apply cps transform to trExpr
        console.log(this.#s.stringify(trExpr))
        // Step 1 is to analyze our variables so we know what to box and what not to box
        let analyzer = new AstAnalysis()
        const ascope = analyzer.analyze(trExpr)
        // Step 2: Contify analysis
        const contifyAnalyzer = new ContifyAnalyzer(ascope)
        const contifyTags = contifyAnalyzer.analyze(trExpr)
        console.log(contifyTags)

        const scope = new CompilerScope(null)
        const nodes: Node[] = []
        const retReg = scope.allocTemp(); // no need to free the temp reg as we return?
        this.#compile(trExpr, { destReg: retReg, nodes, scope, ascope, analyzer })
        const ir = new IR()
        return ir.lower(nodes, scope.numRegs)
    }

    #compile(expr: any, opts: CmpOpts) {
        // Raw values
        if (typeof expr === 'symbol') {
            if (expr === OP_CONT_BASECONT && opts.destReg !== undefined) {
                opts.nodes.push({
                    t: "LoadBaseCont",
                    destReg: opts.destReg
                })
                return
            }
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
                case OP_CONT:
                    this.#compileLambda(expr, opts)
                    return
                case OP_LET:
                case OP_LETSTAR:
                case OP_LETREC:
                case OP_COND:
                    throw new Error("internal error: let/let*/letrec/cond should be transformed by AnimaTransform prior to reaching here")
                case OP_AND:
                case OP_OR:
                    throw new Error("internal error: and/or should be transformed by AstCps prior to reaching here")
            }
        }

        // intrinsic
        const builtinsIdx = IBUILTINS_IDX_MAP.get(operator)
        if (builtinsIdx !== undefined) {
            this.#optIntrinsicNormal(expr, BUILTINS_START+builtinsIdx, opts)
            return
        }

        this.#compileNormalCall(expr, opts)
    }

    #compileBegin(expr: any[], opts: CmpOpts) {
        // We need to load a void if we see an empty begin block
        if (expr.length === 1) {
            if (opts.destReg === undefined) return 
            opts.nodes.push({t: "LoadValue", constant: undefined, destReg: opts.destReg})
            return
        }

        for (let i = 1; i < expr.length; i++) {
            const isLastChild = (i === expr.length - 1);
            this.#compile(expr[i], { ...opts, destReg: isLastChild ? opts.destReg : undefined });
        }
    }

    // compiles both if calls as well as code that is converted into if calls
    #compileIfCall(expr: any[], opts: CmpOpts) {
        if(expr.length != 4) {
            throw new Error(`if condition must be in format ["if", condition, true_expr, false_expr] but only have ${expr.length-1} arguments`)
        }

        // We need to compile the first arg first and leave it to a temp reg
        const condReg = opts.scope.allocTemp()
        this.#compile(expr[1], { ...opts, destReg: condReg })
        // Note that the entire AST is in CPS form so the final end label jump isnt needed 
        const falseLabel = new JumpLabel()
        opts.nodes.push({t: "CondJump", cond: "False", label: falseLabel, reg: condReg})
        opts.scope.freeTemp(condReg) // we can free the reg here

        // Place true code, which will always fall into a tailcall
        this.#compile(expr[2], opts)

        // Place false code as well as jump to start of false code
        opts.nodes.push({t:"Label", label: falseLabel})
        this.#compile(expr[3], opts)
    }

    #compileQuote(expr: any[], opts: CmpOpts) {
        if(expr.length != 2) {
            throw new Error(`quote must be in format ["quote", expr] but have ${expr.length-1} arguments`)
        }
        if(opts.destReg === undefined) return
        opts.nodes.push({t: "LoadValue", constant: normalizeExpr(expr[1]), destReg: opts.destReg})
    }

    // note that the syntax transformer alr handles defines inside a lambda so
    #compileDefine(expr: any[], opts: CmpOpts) {
        if(typeof expr[1] !== "symbol") throw new Error("internal error: complex defines should be transformed by AnimaTransform prior to reaching here")

        // We need to compile the second arg first and leave it on a temp reg
        const valReg = opts.scope.allocTemp();
        this.#compile(expr[2], { ...opts, destReg: valReg });
        opts.nodes.push({t: "SetGlobal", srcReg: valReg, sym: expr[1]})
        opts.scope.freeTemp(valReg)
        if (opts.destReg !== undefined) {
            opts.nodes.push({t: "LoadValue", destReg: opts.destReg, constant: undefined})
        }
    }

    #compileSet(expr: any[], opts: CmpOpts) {
        // AnimaTransform ensures sets are of correct form

        // We need to compile the second arg first and leave it on a temp reg
        const valReg = opts.scope.allocTemp();
        this.#compile(expr[2], { ...opts, destReg: valReg });
        const res = this.#setVar(expr[1], opts, valReg)
        if(res) opts.nodes.push(...res)
        opts.scope.freeTemp(valReg)
        if (opts.destReg !== undefined) {
            opts.nodes.push({t: "LoadValue", destReg: opts.destReg, constant: undefined})
        }
    }

    #compileLambda(expr: any[], opts: CmpOpts) {
        // AnimaTransform ensures lambdas are of correct form
        const lambdaScope = new CompilerScope(opts.scope)
        const ascope = opts.analyzer.scopeMap.get(expr)
        if (!ascope) throw new Error(`internal error: could not find ascope for expr ${expr}`)
        ascope.dbgPrint()

        const { params, remParams } = unpackLambdaExprArgs(expr, IBUILTINS_IDX_MAP, "lambda")
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

        // Compile lambda body (which will later be evaluated as the result of a continuation)
        this.#compile(wrapMulti(expr.slice(2)), {...opts, destReg: undefined, nodes: lambdaNodes, scope: lambdaScope, ascope })
        const template = new ClosureTemplateIR(params, remParams, lambdaNodes, lambdaScope.numRegs, lambdaScope.upvars);
        opts.nodes.push({t: "NewClosure", template: template, destReg: opts.destReg})
    }

    // a normal call
    #compileNormalCall(expr: any[], opts: CmpOpts) {
        // Try IIFE optimizations
        if(this.#optIIFE(expr, opts)) {
            return
        }

        // We need to compile the proc and place it on its own tempval
        const procReg = opts.scope.allocTemp();
        this.#compile(expr[0], { ...opts, destReg: procReg })

        // Push all arguments to a contiguous reg block
        const nargs = expr.length-1 // [func a b c] -> 5 - 2 = 3 args
        const startReg = opts.scope.regAlloc.allocBlock(nargs);
        for (let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, destReg: startReg+i-1 })
        }

        opts.nodes.push({t: "TailCall", nargs, procReg, startReg})

        opts.scope.regAlloc.freeBlock(startReg, nargs)
        opts.scope.freeTemp(procReg)
    }

    #optIIFE(expr: any[], opts: CmpOpts): boolean {
        // if we have a non-variadic IIFE ((lambda (params...) body) args...), then we can optimize it down
        // to BLOCK/ENDBLOCK instead of doing a whole function call
        const first = expr[0]
        if (Array.isArray(first) && (first[0] === OP_LAMBDA || first[0] === OP_CONT) && Array.isArray(first[1])) {
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
                this.#compile(args[i], { ...opts, destReg: tempReg });
                argRegs.push(tempReg);
            }

            // Now enter block
            opts.scope.enterBlock()
            const seen = new Set<symbol>();
            for(let i = 0; i < params.length; i++) {
                ensureCanBind(params[i], seen, "lambda", IBUILTINS_IDX_MAP)
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

    /** Optimizes intrinsic/builtin ops */ 
    #optIntrinsicNormal(expr: any[], builtinIdx: number, opts: CmpOpts) {
        // Push args
        const nargs = expr.length - 1; // expr - Symbol(op)
        const startReg = opts.scope.regAlloc.allocBlock(nargs);
        for(let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, destReg: startReg + (i - 1) });
        }

        opts.nodes.push({t: "IBuiltinTail", startReg, nargs, builtinIdx})
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
