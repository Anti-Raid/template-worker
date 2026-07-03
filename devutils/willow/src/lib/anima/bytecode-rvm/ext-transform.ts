import { ASP, ASTStringifier, DottedPair, OP_AND, OP_BEGIN, OP_CONT, OP_DEFINE, OP_IF, OP_LAMBDA, OP_OR, OP_QUOTE, OP_SET, wrapMulti } from "../common";
import { AnimaTransformer } from "../syntax-transformer";
import { Compiler } from "./compiler";
import { deepPrint } from "./utils";

type SrcMap = Map<any, any>

// Converts an AST into continuation passing style
export class AstCps {
    constructor() {}
    transform(ast: any) {
        return this.#transform(ast)
    }
    #transform(expr: any) {
        const srcMap = new Map()
        let transformed = T(expr, initialCont, new Map(), srcMap)
        console.log(srcMap)
        return transformed
    }
}

// Helper to safely tag a generated node with its source node
const tag = (srcMap: SrcMap, newNode: any, originalNode: any): any => {
    if (newNode && originalNode) {
        srcMap.set(newNode, originalNode);
    }
}

const CONT_TGT = Symbol("tgt")
const makeContFunc = (cont: any): ((val: any) => any) => {
    // OPTIMIZATION: (lambda (v) (k v)) is equivalent to k
    // letting us avoid the lambda altogether (and so on for chains of makeContFunc)
    //
    // CITATION: Claude for the tag functions trick
    const actualCont = (cont as any)[CONT_TGT] !== undefined ? (cont as any)[CONT_TGT] : cont;
    const f = ((v: any) => [actualCont, v]);
    (f as any)[CONT_TGT] = actualCont 
    return f
} 

const INITIAL_K = Symbol.for("k")
const initialCont = makeContFunc(INITIAL_K) 

const makeDynamicCont = (srcMap: SrcMap, origNode: any, k: (v: any) => any): any => {
    if ((k as any)[CONT_TGT] !== undefined) {
        return (k as any)[CONT_TGT]; 
    }

    const v = symGen("v");
    const dynLambda = [OP_CONT, [v], k(v)];
    tag(srcMap, dynLambda, origNode)
    return dynLambda
}

const T = (e: any, k: (v: any) => any, env: Map<symbol, symbol>, srcMap: SrcMap): any => {
    // T(x, k) = k(x)
    if (isConstOrVar(e)) {
        if (typeof e === "symbol") {
            const convE = env.has(e) ? env.get(e) : e
            return k(convE)
        }
        return k(e)
    }
    // basically another 'literal'
    if (e[0] === OP_QUOTE) {
        return k(e[1]);
    }

    if (typeof e[0] === "symbol" && env.has(e[0])) {
        // Treat as normal func call
        return cpsList(e, env, srcMap, (evaluated_exprs) => {
            const dynK = makeDynamicCont(srcMap, e, k);
            return [evaluated_exprs[0], dynK, ...evaluated_exprs.slice(1)]; 
        })
    }

    if (e[0] === OP_IF) {
        // handle ifs here
        const cond = e[1];
        const cons = e[2];
        const alt = e[3];
    
        // We reuse the same `k` for both branches so to avoid duplicating `k`:
        const dynKAst = makeDynamicCont(srcMap, e, k); 
        const dynKFunc = makeContFunc(dynKAst)
        return T(cond, (cond_val) => {
            return [OP_IF, cond_val, T(cons, dynKFunc, env, srcMap), T(alt, dynKFunc, env, srcMap)];
        }, env, srcMap);
    } else if (e[0] === OP_BEGIN) {
        // handle begin here
        const exprs = e.slice(1);
        
        // Edge case: (begin) evaluates to void/null
        if (exprs.length === 0) return k([]) 

        // Helper to sequence the expressions
        const cpsBegin = (exps: any[]): any => {
            // last expr is handled by k
            if (exps.length === 1) {
                return T(exps[0], k, env, srcMap);
            }
            
            // the rest get dummy conts
            return T(exps[0], (_) => {
                return cpsBegin(exps.slice(1));
            }, env, srcMap);
        };
        
        return cpsBegin(exprs)
    } else if (e[0] === OP_LAMBDA) {
        const lambdaEnv = new Map(env);
        const args = e[1];
        const body = wrapMulti(e.slice(2));
        const k_sym = symGen("kl");

        let new_args;

        const mapArgs = (argSym: any) => {
            if(typeof argSym !== "symbol") {
                throw new Error(`lambda parameter must be a symbol, but received ${typeof argSym}: ${String(argSym)}`);
            }

            const uniqueSym = symGen("a")
            lambdaEnv.set(argSym, uniqueSym) 
            return uniqueSym
        }

        if (Array.isArray(args)) {
            // Simple case: we can drive k upwards
            //
            // We do need to rename the args tho for uniqueness purposes
            const mappedArgs = args.map(mapArgs);

            new_args = [k_sym, ...mappedArgs];
        } else if (args instanceof DottedPair) {
            // (lambda (a . b) ...) gets kl stiched on front (lambda (kl a . b) ...)
            const reqArgs = args.items.map(mapArgs)
            const rest = mapArgs(args.rest)
            new_args = new DottedPair([k_sym, ...reqArgs], rest); 
        } else if (typeof args === "symbol") {
            // (lambda args ...) becomes a dotted pair too such as (lambda (kl . args) ...)
            const mappedArg = mapArgs(args)
            new_args = new DottedPair([k_sym], mappedArg)
        } else {
            throw new Error("internal error: cannot transform lambda with symbol(args) to CPS")
        }

        const bodyCPS = T(body, makeContFunc(k_sym), lambdaEnv, srcMap)
        const lambdaCps = [OP_LAMBDA, new_args, bodyCPS]
        tag(srcMap, lambdaCps, e) // tag the new cps lambda so users can debug where bits of the cps ir come from
        return k(lambdaCps)
    } else if (e[0] === OP_DEFINE) {
        const sym = e[1];
        const expr = e[2];     
        return T(expr, (val_sym) => {
            return [OP_BEGIN, [OP_DEFINE, sym, val_sym], k(undefined)];
        }, env, srcMap);
    } else if (e[0] === OP_SET) {
        const sym = e[1];
        const expr = e[2];
        const key_sym = env.has(sym) ? env.get(sym) : sym;
        return T(expr, (val_sym) => {
            return [OP_BEGIN, [OP_SET, key_sym, val_sym], k(undefined)];
        }, env, srcMap);
    } else if (e[0] === OP_AND) {
        const exprs = e.slice(1);
        
        if (exprs.length === 0) return k(true);      // (and) evaluates to #t
        if (exprs.length === 1) return T(exprs[0], k, env, srcMap); // (and e) evaluates to e
        
        const first = exprs[0];
        const rest = [OP_AND, ...exprs.slice(1)];
        
        const dynKAst = makeDynamicCont(srcMap, e, k);
        const dynKFunc = makeContFunc(dynKAst);

        return T(first, (val_sym) => {
            // if true, evaluate the rest with k, otherwise short-circuit and pass the falsy value directly to k.
            return [OP_IF, val_sym, T(rest, dynKFunc, env, srcMap), dynKFunc(val_sym)]
        }, env, srcMap)
    } else if (e[0] === OP_OR) {
        const exprs = e.slice(1);
        
        if (exprs.length === 0) return k(false);     // (or) evaluates to #f
        if (exprs.length === 1) return T(exprs[0], k, env, srcMap); // (or e) evaluates to e
        
        const first = exprs[0];
        const rest = [OP_OR, ...exprs.slice(1)];        
        const dynKAst = makeDynamicCont(srcMap, e, k);
        const dynKFunc = makeContFunc(dynKAst);

        return T(first, (val_sym) => {
            // if false, evaluate the rest with k, otherwise short-circuit and pass the turthy value directly to k.
            return [OP_IF, val_sym, dynKFunc(val_sym), T(rest, dynKFunc, env, srcMap)]
        }, env, srcMap)
    }

    // func call expansion, puts k at the start of the cps list
    return cpsList(e, env, srcMap, (evaluated_exprs) => {
        const dynK = makeDynamicCont(srcMap, e, k);
        return [evaluated_exprs[0], dynK, ...evaluated_exprs.slice(1)]; 
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
const cpsList = (exprs: any[], env: Map<symbol, symbol>, srcMap: SrcMap, buildApplication: (vals: any[]) => any): any => {
    if (exprs.length === 0) {
        return buildApplication([]);
    }
    
    const first = exprs[0];
    const rest = exprs.slice(1);
    
    // If its a constant/variable, just use the variable without applying a cont
    if (isConstOrVar(first)) {
        if (typeof first === "symbol") {
            const convFirst = env.has(first) ? env.get(first) : first
            return cpsList(rest, env, srcMap, (rest_vals) => buildApplication([convFirst, ...rest_vals]));
        }

        return cpsList(rest, env, srcMap, (rest_vals) => buildApplication([first, ...rest_vals]));
    }
    
    // Otherwise, transform the first expression, and in its continuation, do the rest
    return T(first, (val_sym) => {
        return cpsList(rest, env, srcMap, (rest_vals) => buildApplication([val_sym, ...rest_vals]))
    }, env, srcMap)
}

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

(make-account 100)`

(define (make-counter)
  (let ((count 0))
    (lambda ()
      (set! count (+ count 1))
      count)))

(let ((counter-a (make-counter))
      (counter-b (make-counter)))
  (counter-a) ; 1
  (counter-a) ; 2
  (counter-b) ; 1 (Should be completely independent)
  (counter-a))
*/
const simpleProg = `(define fact
(lambda (n)
(if (zero? n)
1
(* n (fact (- n 1))))))`
console.log("Started")
const t1 = performance.now()
const baseAst = new ASP(simpleProg, true).parse()
const synTrans = new AnimaTransformer().transform(baseAst)
//const synTransStr = new ASTStringifier().stringify(synTrans)
//console.log(synTransStr)
const cpsTrans = new AstCps().transform(synTrans)
const astStr = new ASTStringifier().stringify(cpsTrans)
const t2 = performance.now()
console.log(astStr)
console.log(`Took ${t2 - t1} ms`)
const bc = new Compiler().compileAst(cpsTrans)
console.log(deepPrint(bc))