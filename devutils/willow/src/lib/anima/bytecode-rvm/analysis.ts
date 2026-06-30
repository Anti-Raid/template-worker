import {
  OP_QUOTE,
  DottedPair,
  OP_LAMBDA,
  OP_SET,
  OP_CALL_CC
} from "../common";
import { AnalysisScope, BoolRef } from "./scope";

// Analyzes a fully transformed AST to handle scoping prior to actual compilation. This lets us avoid boxing of primitives
export class AstAnalysis {
    scopeMap = new WeakMap<any[], AnalysisScope>();
    
    analyze(ast: any[], callCCEnabled: boolean) {
        const baseScope = new AnalysisScope(null, new BoolRef(callCCEnabled))
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
                const params = ast[1];
                const body = ast.slice(2);
                
                const lambdaScope = new AnalysisScope(scope, scope.callCCEnabled);
                
                for (const p of params) {
                    lambdaScope.define(p); 
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
