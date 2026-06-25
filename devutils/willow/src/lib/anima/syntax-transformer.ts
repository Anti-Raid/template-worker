import {
  OP_DEFINE,
  OP_BEGIN,
  OP_LAMBDA,
  OP_LET,
  OP_IF,
  OP_COND,
  OP_ELSE,
  OP_QUOTE,
  OP_SET,
  OP_LETREC,
  OP_LETSTAR,
  DottedPair,
} from "./common";

/**
 * transforms all of the complex special forms (like cond/let/let-star/letrec/complex defines into their simplest forms (if/lambda/lambda with set!))
 */
export class AnimaTransformer {
    transform(ast: any): any {
        return this.#transform(ast)
    }

    #transform(ast: any, ctx?: string): any {
        if (ast instanceof DottedPair) {
            ast.items = ast.items.map(i => this.#transform(i));
            ast.rest = this.#transform(ast.rest);
            return ast;
        }

        if (Array.isArray(ast) && ast.length >= 0) {
            const op = ast[0];
            if (op === OP_QUOTE) return ast; // cannot desugar a quote
            
            switch (op) {
                case OP_QUOTE: 
                    return ast; 
                case OP_COND:
                    // First transform to ifs', then transform the inner
                    return this.#transform(this.#transformCond(ast));
                case OP_DEFINE:
                    // First desugar the complex defines
                    const [defResult, modified] = this.#transformDefineComplex(ast);
                    if (modified) {
                        // Complex -> normal define
                        return this.#transform(defResult)
                    }
                    ast = defResult as any[];
                    break
                case OP_LET:
                    return this.#transform(this.#transformLet(ast));
                case OP_LETSTAR:
                    return this.#transform(this.#transformLetStar(ast));
                case OP_LETREC:
                    return this.#transform(this.#transformLetrec(ast));
                case OP_LAMBDA:
                    return this.#transformLambda(ast);
            }

            return ast.map((i: any) => this.#transform(i));
        }

        // if no transformations apply, just return the original ast
        return ast
    }

    #wrapMulti = (exprs: any[]) => {
        if (exprs.length === 0) return []; 
        if (exprs.length === 1) return exprs[0];
        return [OP_BEGIN, ...exprs];
    }

    #transformCond(expr: any[]) {
        if (expr.length === 1) throw new Error("cond requires at least one clause");

        let result: any = undefined; 

        for (let i = expr.length - 1; i >= 1; i--) {
            const clause = expr[i];
            if (!Array.isArray(clause) || clause.length < 2) {
                throw new Error(`cond clause must be a list of exactly 2 elements: [condition, expr...]`);
            }

            const condition = clause[0];
            const resultExpr = this.#wrapMulti(clause.slice(1));

            if (condition === OP_ELSE) {
                if (i !== expr.length - 1) {
                    throw new Error("else must be the final clause in a cond statement");
                }
                result = resultExpr;
            } else {
                result = [OP_IF, condition, resultExpr, result];
            }
        }

        return result;
    }

    #transformDefineComplex(expr: any[]): [any[], boolean] {
        if(expr.length < 3) {
            throw new Error(`define must be in format ["define" varname arg] or [define (func_name arg1 arg2... argN) body_expr...] but have ${expr.length-1} arguments`)
        }

        if(typeof expr[1] === "symbol") {
            // Normal define
            if(expr.length !== 3) {
                throw new Error(`define must be in format (define varname expr), but received ${expr.length - 1} arguments`);
            }

            return [expr, false]
        } else if (Array.isArray(expr[1])) { 
            // (define (func_name arg1 arg2) body_expr...), this one just gets rewritten to a normal define with lambda
            if (expr[1].length === 0) throw new Error("define: missing function name");
            const funcName = expr[1][0];
            const params = expr[1].slice(1);
            const body = expr.slice(2);
            const equivExpr = [OP_DEFINE, funcName, [OP_LAMBDA, params, ...body]];
            return [equivExpr, true]
        } else if (expr[1] instanceof DottedPair) {
            // (define (func arg1 . rest) body...)
            if (expr[1].items.length === 0) throw new Error("define: missing function name");
            const funcName = expr[1].items[0];
            const params = expr[1].items.slice(1);
            const body = expr.slice(2);
            
            // If it's (define (func . rest)), the lambda args are just the symbol `rest` (which will then bind everything to remParams)
            // If it's (define (func x . rest)), it's a new DottedPair
            const lambdaArgs = params.length === 0 ? expr[1].rest : new DottedPair(params, expr[1].rest);

            const equivExpr = [OP_DEFINE, funcName, [OP_LAMBDA, lambdaArgs, ...body]];
            return [equivExpr, true]
        } else {
            throw new Error(`define: ${String(expr[1])} not symbol or list syntax`)
        }
    }

    #transformLambda(expr: any[]) {
        const args = expr[1]
        const rawBody = expr.slice(2)

        const internalDefines: any[][] = []
        const body: any[] = []
        let isAtTop = true

        for (let stmt of rawBody) {
            // Transform all inner statements
            stmt = this.#transform(stmt, "lambda")

            if (Array.isArray(stmt) && stmt[0] === OP_DEFINE) {
                if (!isAtTop) throw new Error(`Internal define (${expr}) can only exist at the top-level of a lambda`)
                internalDefines.push(stmt)
                continue
            }
            isAtTop = false
            body.push(stmt)
        }

        // If no defines to transform, just return the transformed body
        if (internalDefines.length === 0) {
            return [OP_LAMBDA, args, ...body]
        }
        
        // Extract the names and values from the [OP_DEFINE, name, value] nodes
        // 
        // Then wrap in a letrec which will then be processed into a lambda with set!'s
        const letrecBindings = internalDefines.map(def => [def[1], def[2]])
        const letrecExpr = [OP_LETREC, letrecBindings, ...body]
        const desugaredBody = this.#transform(letrecExpr, "lambda")

        return [OP_LAMBDA, args, desugaredBody]
    }

    #transformLet(expr: any[], ctx?: string) {
        if (expr.length < 3) throw new Error(`let must be in format ["let", [[var expr]...], body...] but only have ${expr.length-1} arguments`);

        let loopName: symbol | null = null;
        let bindingsIdx = 1;

        if (typeof expr[1] === "symbol") {
            loopName = expr[1];
            bindingsIdx = 2;
            if (expr.length < 4) throw new Error(`named let must include bindings and a body`);
        }

        const bindings = expr[bindingsIdx];
        if (!Array.isArray(bindings) && bindings !== null) {
            throw new Error(`${loopName ? "named let" : "let"} bindings must be a list of form [[var expr]...]`);
        }

        const body = expr.slice(bindingsIdx + 1);
        const params: symbol[] = [];
        const exprs: any[] = [];

        if (bindings !== null) {
            for (const binding of bindings) {
                if (!Array.isArray(binding) || binding.length !== 2) {
                    throw new Error(`let binding \`${binding}\` must be a list of form [var expr]`);
                }
                const sym = binding[0];
                const val = binding[1];

                if (typeof sym !== "symbol") throw new Error("let binding name must be a symbol");
                
                params.push(sym);
                exprs.push(val);
            }
        }

        // Expand let/named let down to a lambda
        if (loopName) {
            const namedLetExpr = [
                [
                    OP_LAMBDA, 
                    [], 
                    [OP_DEFINE, loopName, [OP_LAMBDA, params, ...body]],
                    [loopName, ...exprs]
                ]
            ];
            return namedLetExpr;
        } else {
            // rewrite to lambda [(let ((var expr) ...) body1 body2 ...) => ((lambda (var...) body1 body2...) expr...)]
            const equivExpr = [[OP_LAMBDA, params, ...body], ...exprs];
            return equivExpr
        }
    }

    #transformLetStar(expr: any[]) {
        if (expr.length < 3) throw new Error(`let*: bad syntax`);

        const bindings = expr[1];
        if (!Array.isArray(bindings) && bindings !== null) {
            throw new Error(`let* bindings must be a list of form [[var expr]...]`);
        }

        const body = expr.slice(2);

        // No bindings
        if (bindings === null || bindings.length === 0) {
            const equivExpr = [[OP_LAMBDA, [], ...body]];
            return equivExpr
        }

        // Start with innermost expr and work our way outwards (similar to cond)
        let currentExpr = body; 
        for (let i = bindings.length - 1; i >= 0; i--) {
            const binding = bindings[i];
            if (!Array.isArray(binding) || binding.length !== 2) {
                throw new Error(`let* binding \`${binding}\` must be a list of form [var expr]`);
            }
            
            const sym = binding[0];
            const val = binding[1];

            if (typeof sym !== "symbol") throw new Error("let* binding name must be a symbol");

            // Wrap in lambda
            const nextExpr = [
                [OP_LAMBDA, [sym], ...currentExpr], 
                val
            ];
            
            // The result becomes the body for the next outer lambda.
            currentExpr = [nextExpr];
        }

        return currentExpr[0]
    }

    #transformLetrec(expr: any[]) {
        // OP_LETREC is also special like let and can also be translated into a lambda
        if (expr.length < 3) throw new Error(`letrec: bad syntax`);

        const bindings = expr[1];
        if (!Array.isArray(bindings) && bindings !== null) {
            throw new Error(`letrec bindings must be a list of form [[var expr]...]`);
        }

        const body = expr.slice(2);
        const params: symbol[] = [];
        const dummyVals: any[] = []; // The #<void>'s
        const setExprs: any[] = [];  // The (set! p1 v1)

        if (bindings !== null) {
            for (const binding of bindings) {
                if (!Array.isArray(binding) || binding.length !== 2) {
                    throw new Error(`letrec binding \`${binding}\` must be a list of form [var expr]`);
                }
                const sym = binding[0];
                const val = binding[1];

                if (typeof sym !== "symbol") throw new Error("letrec binding name must be a symbol");
                
                params.push(sym);
                dummyVals.push(undefined); 
                setExprs.push([OP_SET, sym, val]); 
            }
        }

        // rewrite to lambda [((lambda (params...) (set! p1 v1)... body...) #<void>...)]
        const equivExpr = [[OP_LAMBDA, params, ...setExprs, ...body], ...dummyVals];
        return equivExpr
    }
}
