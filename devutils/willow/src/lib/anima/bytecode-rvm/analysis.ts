import {
  OP_QUOTE,
  DottedPair,
  OP_LAMBDA,
  OP_SET,
  OP_CONT,
  OP_IF,
  OP_BEGIN,
  OP_DEFINE,
  unpackLambdaExprArgs,
} from "../common";
import { AnalysisScope } from "./scope";

// Analyzes a fully transformed AST to handle scoping prior to actual compilation. This lets us avoid boxing of primitives
export class AstAnalysis {
    scopeMap = new WeakMap<any[], AnalysisScope>();
    
    analyze(ast: any[]) {
        const baseScope = new AnalysisScope(null)
        this.scopeMap.set(ast, baseScope)
        this.visit(ast, baseScope);
        return baseScope
    }

    private visit(ast: any, scope: AnalysisScope) {       
        // Symbols simulate a read into the value
        if (typeof ast === 'symbol') {
            scope.readVar(ast); 
            return;
        }
        
        // Base cases: primitives, strings, symbols, or null
        if (ast === null || typeof ast !== "object") {
            return;
        }

        if (ast instanceof DottedPair) {
            // analyze inner items of dotted pair
            for (const child of ast.items) this.visit(child, scope);
            this.visit(ast.rest, scope);
            return;
        }

        if (!Array.isArray(ast)) return

        const op = ast[0];
        switch (op) {
            case OP_QUOTE:
                return // don't touch quoted
            case OP_LAMBDA:
            case OP_CONT:
                const body = ast.slice(2);
                
                const lambdaScope = new AnalysisScope(scope);
                
                const extractedParams = unpackLambdaExprArgs(ast)
                for (const p of extractedParams.params) {
                    lambdaScope.define(p); 
                }
                if (extractedParams.remParams) {
                    lambdaScope.define(extractedParams.remParams); 
                }

                this.scopeMap.set(ast, lambdaScope);

                // Visit children (yes scope change)
                for (const child of body) this.visit(child, lambdaScope);
                return;
            case OP_SET:
                const sym = ast[1];
                const value = ast[2];
                
                scope.markMutable(sym);
                this.visit(value, scope);
                return;
        }
        // Visit children (no scope change)
        for (const child of ast) {
            this.visit(child, scope);
        }
    }
}

type CallSite = { expr: any, args: any[], isRecursive: boolean, contArg: any }
type ContifyTag = { expr: any, kind: "LOOP" | "JOIN", callSites: CallSite[] }
type Continuation = { params: any, body: any, callSites: CallSite[], escapes: boolean, lambdaExpr: any }

// Must run after AstAnalysis produces AnalysisScope
export class ContifyAnalyzer {
    public candidates = new Map<any, Continuation>();
    public boundNames = new Map<symbol, any>();
    public activeLambdas = new Set<any>();
    public knownContinuationSymbols = new Set<symbol>()

    constructor(public ascope: AnalysisScope) {}

    analyze(ast: any): ContifyTag[] {
        this.visitValue(ast);
        return this.selectContifyTargets();
    }

    private registerCandidate(lambdaExpr: any) {
        if (this.candidates.has(lambdaExpr)) return;
        
        const { params, remParams } = unpackLambdaExprArgs(lambdaExpr)

        if (params && params.length > 0) {
            this.knownContinuationSymbols.add(params[0]);
        }

        const cont: Continuation = { params: { params, remParams }, body: lambdaExpr[2], callSites: [], escapes: false, lambdaExpr }
        this.candidates.set(lambdaExpr, cont)

        this.activeLambdas.add(lambdaExpr)
        this.visitValue(cont.body)
        this.activeLambdas.delete(lambdaExpr)
    }

    private recordCallSite(targetLambda: any, callExpr: any, args: any[], contArg: any) {
        const cont = this.candidates.get(targetLambda);
        if (!cont) return;

        cont.callSites.push({
            expr: callExpr,
            args,
            isRecursive: this.activeLambdas.has(targetLambda),
            contArg
        });
    }

    private markEscaped(targetLambda: any) {
        const cont = this.candidates.get(targetLambda);
        if (cont) cont.escapes = true;
    }

    private isTailCallSite(cont: Continuation, cs: CallSite): boolean {
        const ownCont = cont.params.params?.[0];
        return ownCont !== undefined && cs.contArg === ownCont;
    }

    private visitValue(expr: any) {
        if (!expr || !Array.isArray(expr) || expr.length === 0) {
            if (typeof expr === "symbol" && this.boundNames.has(expr)) {
                this.markEscaped(this.boundNames.get(expr));
            }
            return;
        }

        const op = expr[0];

        switch (op) {
            case OP_QUOTE:
                return // don't touch quoted
            case OP_LAMBDA:
            case OP_CONT:
                // If a lambda OR a continuation hits visitValue, it means it was 
                // found in a argument/value and hence must be marked as escaping
                this.registerCandidate(expr);
                this.markEscaped(expr); 
                break;
            case OP_IF:
                this.visitValue(expr[1]); // cond
                this.visitValue(expr[2]); // truthy
                this.visitValue(expr[3]); // falsy
                break;
            case OP_BEGIN:
                expr.slice(1).forEach((arg: any) => this.visitValue(arg));
                break;
            case OP_DEFINE:
            case OP_SET:
                const [sym, rhs] = [expr[1], expr[2]];
                if (op === OP_DEFINE && this.isEligibleForContification(sym, rhs)) {
                    this.boundNames.set(sym, rhs);
                    this.registerCandidate(rhs);
                } else {
                    this.visitValue(rhs);
                }
                break;
            default:
                // It's a CPS function call: [callee, continuation, ...dataArgs]
                this.visitCPSCall(expr);
                break;
        }
    }

    private visitCPSCall(expr: any[]) {
        const callee = expr[0]

        if (Array.isArray(callee) && callee[0] === OP_CONT) {
            // Literal continuation node in callee position
            const dataArgs = expr.slice(1);
            this.registerCandidate(callee);
            this.recordCallSite(callee, expr, dataArgs, null);
            for (const arg of dataArgs) this.visitValue(arg);
            return;
        }

        if (typeof callee === "symbol" && this.knownContinuationSymbols.has(callee)) {
            // Bare continuation-parameter symbol
            const dataArgs = expr.slice(1);
            for (const arg of dataArgs) this.visitValue(arg);
            return;
        }

        const contArg = expr[1] 
        const dataArgs = expr.slice(2)

        this.visitCallee(callee, expr, dataArgs, contArg)

        if (Array.isArray(contArg) && contArg[0] === OP_CONT) {
            this.registerCandidate(contArg)
            this.recordCallSite(contArg, expr, dataArgs, contArg)
        } else if (typeof contArg === "symbol" && this.boundNames.has(contArg)) {
            this.recordCallSite(this.boundNames.get(contArg), expr, [], contArg)
        } else {
            this.visitValue(contArg)
        }

        for (const arg of dataArgs) {
            this.visitValue(arg); 
        }
    }

    private visitCallee(callee: any, callExpr: any, dataArgs: any[], contArg: any) {
        if (typeof callee === "symbol" && this.boundNames.has(callee)) {
            this.recordCallSite(this.boundNames.get(callee), callExpr, dataArgs, contArg);
        } else if (Array.isArray(callee) && (callee[0] === OP_LAMBDA || callee[0] === OP_CONT)) {
            this.registerCandidate(callee);
            this.recordCallSite(callee, callExpr, dataArgs, contArg);
        } else {
            this.visitValue(callee);
        }
    }

    private isEligibleForContification(sym: symbol, rhs: any): boolean {
        return Array.isArray(rhs) && 
               (rhs[0] === OP_LAMBDA || rhs[0] === OP_CONT) && 
               !this.ascope.getVarinfo(sym)?.mutable;
    }

    private selectContifyTargets(): ContifyTag[] {
        const results: ContifyTag[] = [];
        for (const [expr, cont] of this.candidates) {
            if (cont.escapes) continue;

            const internalCalls = cont.callSites.filter(cs => cs.isRecursive)
            const hasInternalCall = internalCalls.length > 0
            const isSingleExternalCall = cont.callSites.length === 1 && !hasInternalCall;

            if (isSingleExternalCall) continue; // IIFE
            if (hasInternalCall && !internalCalls.every(cs => this.isTailCallSite(cont, cs))) {
                continue // non-tail recursion cannot be contified
            }

            if (cont.callSites.length > 0) {
                results.push({
                    expr,
                    kind: hasInternalCall ? "LOOP" : "JOIN",
                    callSites: cont.callSites
                });
            }
        }
        return results;
    }
}