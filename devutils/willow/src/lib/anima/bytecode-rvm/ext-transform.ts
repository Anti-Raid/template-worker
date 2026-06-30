import { ASP, ASTStringifier, DottedPair, OP_AND, OP_BEGIN, OP_DEFINE, OP_IF, OP_LAMBDA, OP_OR, OP_QUOTE, OP_SET } from "../common";
import { AnimaTransformer } from "../syntax-transformer";
import { Compiler } from "./compiler";
import { deepPrint } from "./utils";

// Converts an AST into continuation passing style
export class AstCps {
    constructor() {}
    transform(ast: any) {
        return this.#transform(ast)
    }
    #transform(expr: any) {
        return T(expr, Symbol.for("k"))
    }
}

const wrapMulti = (exprs: any[]) => {
    if (exprs.length === 0) return []; 
    if (exprs.length === 1) return exprs[0];
    return [OP_BEGIN, ...exprs];
}

const T = (e: any, k: any): any => {
    // T(x, k) = k(x)
    if (isConstOrVar(e)) {
        return [k, e]
    }
    // basically another 'literal'
    if (e[0] === OP_QUOTE) {
        return [k, e[1]];
    }

    if (e[0] === OP_IF) {
        // handle ifs here
        const cond = e[1];
        const cons = e[2];
        const alt = e[3];
    
        // We reuse the same `k` for both branches
        const cond_val = symGen("cond_val");
        
        return T(cond, [
            OP_LAMBDA, 
            [cond_val],
            [OP_IF, cond_val, T(cons, k), T(alt, k)]
        ]);
    } else if (e[0] === OP_BEGIN) {
        // handle begin here
        const exprs = e.slice(1);
        
        // Edge case: (begin) evaluates to void/null
        if (exprs.length === 0) return [k, []]; 

        // Helper to sequence the expressions
        const cpsBegin = (exps: any[]): any => {
            // last expr is handled by k
            if (exps.length === 1) {
                return T(exps[0], k);
            }
            
            const dummy_sym = symGen("dk"); // dummy cont
            return T(exps[0], [
                OP_LAMBDA, 
                [dummy_sym], 
                cpsBegin(exps.slice(1))
            ]);
        };
        
        return cpsBegin(exprs)
    } else if (e[0] === OP_LAMBDA) {
        const args = e[1];
        const body = wrapMulti(e.slice(2));
        const k_sym = symGen("kl");

        let new_args;

        if (Array.isArray(args)) {
            // Simple case: we can drive k upwards
            new_args = [k_sym, ...args];
        } else if (args instanceof DottedPair) {
            // (lambda (a . b) ...) gets kl stiched on front (lambda (kl a . b) ...)
            new_args = new DottedPair([k_sym, ...args.items], args.rest); 
        } else if (typeof args === "symbol") {
            // (lambda args ...) becomes a dotted pair too such as (lambda (kl . args) ...)
            new_args = new DottedPair([k_sym], args);
        } else {
            throw new Error("internal error: cannot transform lambda with symbol(args) to CPS")
        }

        return [k, [OP_LAMBDA, new_args, T(body, k_sym)]];
    } else if (e[0] === OP_DEFINE) {
        const sym = e[1];
        const expr = e[2];
        
        const val_sym = symGen("d");
        
        // Transform RHS
        return T(expr, [
            OP_LAMBDA,
            [val_sym],
            [OP_BEGIN, 
                [OP_DEFINE, sym, val_sym], 
                [k, undefined] // The return value of `define` is void
            ]
        ]);
    } else if (e[0] === OP_SET) {
        const sym = e[1];
        const expr = e[2];
        
        const val_sym = symGen("s");
        
        // Transform RHS
        return T(expr, [
            OP_LAMBDA,
            [val_sym],
            [OP_BEGIN, 
                [OP_SET, sym, val_sym], 
                [k, undefined] // The return value of `define` is void
            ]
        ]);
    } else if (e[0] === OP_AND) {
        const exprs = e.slice(1);
        
        if (exprs.length === 0) return [k, true];      // (and) evaluates to #t
        if (exprs.length === 1) return T(exprs[0], k); // (and e) evaluates to e
        
        const first = exprs[0];
        const rest = [OP_AND, ...exprs.slice(1)];
        const val_sym = symGen("and_val");
        
        return T(first, [
            OP_LAMBDA,
            [val_sym],
            [OP_IF, val_sym, T(rest, k), [k, val_sym]] // if true, evaluate the rest with k, otherwise short-circuit and pass the falsy value directly to k.
        ]);
    } else if (e[0] === OP_OR) {
        const exprs = e.slice(1);
        
        if (exprs.length === 0) return [k, false];     // (or) evaluates to #f
        if (exprs.length === 1) return T(exprs[0], k); // (or e) evaluates to e
        
        const first = exprs[0];
        const rest = [OP_OR, ...exprs.slice(1)];
        const val_sym = symGen("or_val");
        
        return T(first, [
            OP_LAMBDA,
            [val_sym],
            [OP_IF, val_sym, [k, val_sym], T(rest, k)] // opposite of AND
        ]);
    }

    // func call expansion, puts k at the start of the cps list
    return cpsList(e, (evaluated_exprs) => {
        return [evaluated_exprs[0], k, ...evaluated_exprs.slice(1)]; 
    });
}

const isConstOrVar = (expr: any) => {
    if (expr instanceof DottedPair) {
        return true
    } else if (!Array.isArray(expr)) { // non array (symbol, string etc.)
        return true // don't translate raw literals
    }

    if (expr.length === 0) {
        // An empty array evaluates to null which is *also* a literal
        return true
    }

    return false
}

// Helper to transform a list of expressions sequentially
const cpsList = (exprs: any[], buildApplication: (vals: any[]) => any): any => {
    if (exprs.length === 0) {
        return buildApplication([]);
    }
    
    const first = exprs[0];
    const rest = exprs.slice(1);
    
    // If its a constant/variable, just use the variable without applying a cont
    if (isConstOrVar(first)) {
        return cpsList(rest, (rest_vals) => buildApplication([first, ...rest_vals]));
    }
    
    // Otherwise, transform the first expression, and in its continuation, do the rest
    const val_sym = symGen("val");
    return T(first, [
        OP_LAMBDA, 
        [val_sym], 
        cpsList(rest, (rest_vals) => buildApplication([val_sym, ...rest_vals]))
    ]);
};

let n = 0
const symGen = (base: string) => {
    return Symbol.for(`${base}${n++}`)
}

// test
/*const simpleProg = `(define (factorial n)
 (if (= n 0)
     1     ; NOT tail-recursive
     (* n (factorial (- n 1)))))`*/
/*const simpleProg = `
(define (make-account initial)
  (let ((balance initial))
    (define (withdraw amount)
      (set! balance (- balance amount))
      balance)
    (define (deposit amount)
      (set! balance (+ balance amount))
      balance)
    (withdraw 10)
    (deposit 50)))

(make-account 100)`*/
const simpleProg = `(define (make-counter)
  (let ((count 0))
    (lambda ()
      (set! count (+ count 1))
      count)))

(let ((counter-a (make-counter))
      (counter-b (make-counter)))
  (counter-a) ; 1
  (counter-a) ; 2
  (counter-b) ; 1 (Should be completely independent)
  (counter-a))`
const t1 = performance.now()
const baseAst = new ASP(simpleProg, true).parse()
const synTrans = new AnimaTransformer().transform(baseAst)
const synTransStr = new ASTStringifier().stringify(synTrans)
//console.log(synTransStr)
const cpsTrans = new AstCps().transform(synTrans)
const astStr = new ASTStringifier().stringify(cpsTrans)
const t2 = performance.now()
console.log(astStr)
console.log(`Took ${t2 - t1} ms`)
const bc = new Compiler().compile(astStr)
console.log(deepPrint(bc))