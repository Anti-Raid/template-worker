import { Cons } from "./list";

export class MissingVarError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'MissingVarError';
    }
}

class AnimaScope {
    #data: Record<string, any>; // from svelte $state etc
    #outer: AnimaScope | null;

    /** gas/steps the vm has taken. Each vm eval loop takes 1 step, JS funcs can also increment steps etc as they desire */
    state: {steps: number}; 

    constructor(data: Record<string, any>, outer: AnimaScope | null, state: {steps: number}) {
        this.#data = data
        this.#outer = outer
        this.state = state
    }

    nest(): AnimaScope {
        // Nested scopes don't need to be reactive
        return new AnimaScope(Object.create(null), this, this.state);
    }

    get(key: symbol): any {
        const skey = key.description // Symbol.keyFor(key); 
        if (!skey) throw new Error(`Internal error: could not find symbol for ${String(key)}`);

        let scope: AnimaScope | null = this
        while(scope) {
            if (Object.prototype.hasOwnProperty.call(scope.#data, skey)) {
                return scope.#data[skey];
            }            
            scope = scope.#outer
        }
        throw new MissingVarError(`Variable '${skey}' is not defined in the current scope.`);
    }

    define(key: symbol, value: any) {
        if (this.#outer === null) throw new Error("Cannot set key on global scope")
        const skey = key.description // Symbol.keyFor(key); 
        if (!skey) throw new Error(`Internal error: could not find symbol for ${String(key)}`);
        this.#data[skey] = value;
    }
}

/** JS Closure */
export class Closure {
    params: symbol[];
    body: any;
    scope: AnimaScope;

    constructor(params: symbol[], body: any, scope: AnimaScope) {
        this.params = params
        this.body = body
        this.scope = scope
    }
}

// Special Forms
export const OP_DEFINE = Symbol.for("define");
export const OP_BEGIN     = Symbol.for("begin");
export const OP_LAMBDA = Symbol.for("lambda");
export const OP_LET    = Symbol.for("let");
export const OP_IF     = Symbol.for("if");
export const OP_COND   = Symbol.for("cond");
export const OP_ELSE   = Symbol.for("else");
export const OP_QUOTE  = Symbol.for("quote");
export const OP_AND      = Symbol.for("and");
export const OP_OR       = Symbol.for("or");

// List Operations
export const OP_LIST     = Symbol.for("list");
export const OP_CONS     = Symbol.for("cons")
export const OP_CAR      = Symbol.for("car");
export const OP_CDR      = Symbol.for("cdr");
export const OP_LAST     = Symbol.for("last");
export const OP_LENGTH   = Symbol.for("length");
export const OP_EMPTY    = Symbol.for("empty?")
export const OP_CONTAINS = Symbol.for("contains");
export const OP_MAP      = Symbol.for("map")
export const OP_APPLY    = Symbol.for("apply")

// Logic & Type Checking
export const OP_NOT      = Symbol.for("not");
export const OP_TYPE     = Symbol.for("type?");
export const OP_EQ       = Symbol.for("=");
export const OP_EQ_PTR1  = Symbol.for("eq?");
export const OP_EQ_PTR2  = Symbol.for("eqv?");
export const OP_EQ_DEEP1 = Symbol.for("equal?");
export const OP_EQ_DEEP2 = Symbol.for("equals?");

// Math & Comparisons
export const OP_LT     = Symbol.for("<");
export const OP_GT     = Symbol.for(">");
export const OP_LTE    = Symbol.for("<=");
export const OP_GTE    = Symbol.for(">=");
export const OP_ADD    = Symbol.for("+");
export const OP_SUB    = Symbol.for("-");
export const OP_MUL    = Symbol.for("*");
export const OP_DIV    = Symbol.for("/");
export const OP_MODULO = Symbol.for("modulo");

export const SPECIAL_FORMS = new Set([
    OP_DEFINE, 
    OP_QUOTE, 
    OP_LAMBDA, 
    OP_IF, 
    OP_COND, 
    OP_ELSE, 
    OP_AND, 
    OP_OR, 
    OP_BEGIN
])

/** A builtin method in anima */
export class Builtin {
    cb: (vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => any
    constructor(cb: (vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => any) {
        this.cb = cb
    }
}

const strictEqProc = new Builtin((vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => {
    if (argCount != 2) throw new Error(`${String(expr[0])} requires exactly 2 arguments`);
    return Object.is(vm.evalinner(expr[1], scope), vm.evalinner(expr[2], scope));
});

const deepEqProc = new Builtin((vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => {
    if (argCount != 2) throw new Error(`${String(expr[0])} requires exactly 2 arguments`);
    const left = vm.evalinner(expr[1], scope);
    const right = vm.evalinner(expr[2], scope);
    return vm.isDeepEqual(left, right); 
})

const createMathOp = (operatorName: string, op: (a: number, b: number) => number) => {
    return new Builtin((vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => {
        if (argCount < 2) throw new Error(`${operatorName} requires at least 2 arguments`);
        
        let result = vm.evalinner(expr[1], scope);
        if (typeof result !== "number") throw new Error(`${operatorName} requires numbers`);
        
        for (let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i + 1], scope);
            if (typeof next !== "number") throw new Error(`${operatorName} requires numbers`);
            result = op(result, next);
        }
        return result;
    });
}

const createCompareOp = (operatorName: string, op: (a: number, b: number) => boolean) => {
    return new Builtin((vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => {
        if (argCount < 2) throw new Error(`${operatorName} requires at least 2 arguments`);
        
        let prev = vm.evalinner(expr[1], scope);
        if (typeof prev !== "number") throw new Error(`${operatorName} requires numbers`);
        
        for (let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i + 1], scope);
            if (typeof next !== "number") throw new Error(`${operatorName} requires numbers`);
            
            if (!op(prev, next)) return false;
            prev = next;
        }
        return true;
    });
}

/** Builtin procedures */
export const BUILTIN_PROCS: Record<symbol, Builtin> = {
    [OP_ADD]: createMathOp("+", (a, b) => a + b),
    [OP_SUB]: createMathOp("-", (a, b) => a - b),
    [OP_MUL]: createMathOp("*", (a, b) => a * b),
    [OP_DIV]: createMathOp("/", (a, b) => a / b),
    [OP_MODULO]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 2) throw new Error("modulo requires 2 arguments");
        return vm.evalinner(expr[1], scope) % vm.evalinner(expr[2], scope);
    }),
    [OP_LIST]: new Builtin((vm, argCount, expr, scope) => {
        const lst = new Array(argCount);
        for (let i = 0; i < argCount; i++) {
            lst[i] = vm.evalinner(expr[i+1], scope);
        }
        return lst;
    }),
    [OP_CONS]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 2) throw new Error("cons requires 2 arguments [cons a d]");
        const a = vm.evalinner(expr[1], scope);
        const d = vm.evalinner(expr[2], scope);
        return Cons.pair(a, d)
    }),
    [OP_CAR]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 1) throw new Error("car requires 1 argument");
        const val = vm.evalinner(expr[1], scope);
        if (Array.isArray(val)) {
            if (val.length < 1) throw new Error("car requires a non-empty list");
            return val[0];
        } else if (val instanceof Cons) {
            return val.head;
        } else if (val === null) {
            throw new Error("car requires a non-empty list");
        } else {
            throw new Error("car requires a list");
        }
    }),
    [OP_CDR]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 1) throw new Error("cdr requires 1 argument");
        const val = vm.evalinner(expr[1], scope);
        if (Array.isArray(val)) {
            if (val.length < 1) throw new Error("cdr requires a non-empty list");
            return Cons.fromArray(val, 1)
        } else if (val instanceof Cons) { 
            return val.tail
        } else if (val === null) {
            throw new Error("car requires a non-empty list");
        } else {
            throw new Error(`cdr requires a list but got ${val}`);
        }
    }),
    [OP_LAST]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 1) throw new Error("last requires 1 argument");
        const val = vm.evalinner(expr[1], scope);
        if (Array.isArray(val)) {
            if (val.length < 1) throw new Error("last requires a non-empty list");
            return val[val.length - 1];
        } else if (val instanceof Cons) {
            let last = val.head
            for(const v of val) {
                last = v;
            }
            return last
        } else if (val === null) {
            throw new Error("last requires a non-empty list");
        } else {
            throw new Error("last requires a list");
        }
    }),
    [OP_LENGTH]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 1) throw new Error("length requires 1 argument");
        const target = vm.evalinner(expr[1], scope);
        if (target === null) return 0
        return (Array.isArray(target) || target instanceof Cons) ? target.length : (typeof target === "string" ? target.length : 0);
    }),
    [OP_EMPTY]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 1) throw new Error("empty? requires 1 argument");
        const target = vm.evalinner(expr[1], scope);
        if (target === null) return true
        return (Array.isArray(target) || target instanceof Cons) ? (target.length == 0) : (typeof target === "string" ? (target.length == 0) : false);
    }),
    [OP_CONTAINS]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 2) throw new Error("contains requires 2 arguments");
        const list = vm.evalinner(expr[1], scope);
        const item = vm.evalinner(expr[2], scope);
        return (Array.isArray(list) || list instanceof Cons) ? list.includes(item) : false;
    }),
    [OP_MAP]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount < 2) throw new Error("map requires at least 2 arguments (procedure and 1+ lists to map over)");
        
        const proc = vm.evalinner(expr[1], scope);
        const lists: any[] = [];
        
        for (let i = 2; i <= argCount; i++) {
            lists.push(vm.evalinner(expr[i], scope));
        }

        const iters = lists.map(list => {
            if (list === null) return [][Symbol.iterator]();
            if (Array.isArray(list) || list instanceof Cons) return list[Symbol.iterator]();
            throw new Error("map arguments must be lists");
        });

        const result = [];        
        while (true) {
            const nextVals = iters.map(it => it.next());
            
            if (nextVals.some(res => res.done)) {
                break;
            }
            const args = [];
            for (const res of nextVals) {
                args.push(res.value);
            }
            result.push(vm.execproc(proc, args, scope));
        }
        
        return result;
    }),
    [OP_APPLY]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount < 2) throw new Error("apply requires at least 2 arguments (apply procedure [arg...] lst)");
        
        const proc = vm.evalinner(expr[1], scope);
        const args = [];
        
        for (let i = 2; i <= argCount; i++) {
            args.push(vm.evalinner(expr[i], scope));
        }

        // Last arg must be the list of remaining args
        const lst = args.pop();
        
        if (lst !== null) {
            if (Array.isArray(lst) || lst instanceof Cons) {
                for (const item of lst) {
                    args.push(item);
                }
            } else {
                throw new Error("apply: the last argument must be a list (apply procedure [arg...] lst)");
            }
        }

        return vm.execproc(proc, args, scope);
    }),
    [OP_NOT]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 1) throw new Error("not requires 1 argument");
        return !vm.isTruthy(vm.evalinner(expr[1], scope));
    }),
    [OP_EQ_PTR1]: strictEqProc,
    [OP_EQ_PTR2]: strictEqProc,
    [OP_EQ_DEEP1]: deepEqProc,
    [OP_EQ_DEEP2]: deepEqProc,

    // comparison operators
    [OP_LT]:  createCompareOp("<",  (a, b) => a < b),
    [OP_GT]:  createCompareOp(">",  (a, b) => a > b),
    [OP_LTE]: createCompareOp("<=", (a, b) => a <= b),
    [OP_GTE]: createCompareOp(">=", (a, b) => a >= b),
    [OP_EQ]:  createCompareOp("=",  (a, b) => a === b),

    [OP_TYPE]: new Builtin((vm, argCount, expr, scope) => {
        if(argCount != 1) {
            throw new Error(`type? must be in format ["type?", expr] but only have ${argCount} arguments`)
        }

        if (typeof expr[1] === "symbol" && SPECIAL_FORMS.has(expr[1])) {
            throw new Error(`${String(expr[1])}: bad syntax`)
        }

        const resolvedValue = vm.evalinner(expr[1], scope);
        if (resolvedValue === null) return "list";
        switch(typeof resolvedValue) {
            case "string": return "string"
            case "number": return "number"
            case "boolean": return "boolean"
            case "undefined": return "null"
            case "symbol": return "symbol";
            default: {
                if (resolvedValue instanceof Builtin) return "procedure";
                if(resolvedValue instanceof Closure) return "procedure"
                if(Array.isArray(resolvedValue) || resolvedValue instanceof Cons) return "list"
                return "object" // to allow consistency across all js engines/custom sv2 impls etc.
            }
        }
    }),
    // @ts-ignore
    __proto__: null
}

/** A special form in anima */
export class SpecialForm {
    // cs is current vm expr state and can be set to allow for tco exec in a special form
    cb: (vm: Anima, argCount: number, expr: any[], scope: AnimaScope, cs: {expr: any}) => any
    constructor(cb: (vm: Anima, argCount: number, expr: any[], scope: AnimaScope, cs: {expr: any}) => any) {
        this.cb = cb
    }
}
export const SPECIAL_FORM_TCO_TRIGGER = Symbol("tco")

export const SPECIAL_FORM_PROCS: Record<symbol, SpecialForm> = {
    [OP_DEFINE]: new SpecialForm((vm, argCount, expr, scope, cs) => {
        if (vm.disableDefine) {
            throw new Error("define expressions are disabled in this context");
        }

        if(argCount < 2) {
            throw new Error(`define must be in format ["define" varname arg] or [define (func_name arg1 arg2... argN) body_expr...] but have ${argCount} arguments`)
        }

        // Normal define
        if(typeof expr[1] === "symbol") {
            if (SPECIAL_FORMS.has(expr[1])) {
                throw new Error(`${String(expr[1])}: bad syntax`)
            }
            if (expr[1] in BUILTIN_PROCS) {
                throw new Error(`${String(expr[1])}: cannot shadow builtin procedure`)
            }

            const val = vm.evalinner(expr[2], scope);
            scope.define(expr[1], val);
            return undefined;
        } else if (Array.isArray(expr[1])) {
            // (define (func_name arg1 arg2) body_expr...), this one just gets rewritten to a normal define with lambda
            if (expr[1].length === 0) throw new Error("define: missing function name");
            const funcName = expr[1][0];
            const params = expr[1].slice(1);
            const body = expr.slice(2);
            cs.expr = [OP_DEFINE, funcName, [OP_LAMBDA, params, ...body]];
            return SPECIAL_FORM_TCO_TRIGGER
        } else if (expr[1] instanceof Cons) {
            // same as above
            const funcName = expr[1].head;
            const params = expr[1].tail;
            if (!params) throw new Error("define: missing params");
            const body = expr.slice(2);
            cs.expr = [OP_DEFINE, funcName, [OP_LAMBDA, params, ...body]];
            return SPECIAL_FORM_TCO_TRIGGER
        } else {
            throw new Error(`${String(expr[1])}: expr[1] not symbol or list syntax`)
        }
    }),
    [OP_BEGIN]: new SpecialForm((vm, argCount, expr, scope, cs) => {
        if(argCount == 0) {
            return undefined // evaluates to void
        }

        for (let i = 0; i < argCount - 1; i++) {
            vm.evalinner(expr[i+1], scope);
        }

        cs.expr = expr[argCount];
        return SPECIAL_FORM_TCO_TRIGGER
    }),
    [OP_LAMBDA]: new SpecialForm((vm, argCount, expr, scope, cs) => {
        if(vm.disableLambda) {
            throw new Error("lambda expressions are disabled in this context")
        }

        if(argCount < 2) {
            throw new Error(`lambda must be in format ["lambda", [bind-args...], body...] but only have ${argCount} arguments`)
        }

        let elems;
        if (Array.isArray(expr[1])) {
            elems = expr[1]
        } else if (expr[1] instanceof Cons) {
            elems = []
            for (const p of expr[1]) {
                elems.push(p);
            }
        } else {
            throw new Error(`lambda parameters must be a list`);
        }

        // Validate that every parameter is a symbol
        for(let i = 0; i < elems.length; i++) {
            if(typeof elems[i] !== "symbol") {
                throw new Error(`lambda parameter at index ${i} must be a symbol, but received ${typeof elems[i]}: ${String(elems[i])}`);
            }
            if (SPECIAL_FORMS.has(elems[i])) {
                throw new Error(`${String(elems[i])}: bad syntax`)
            }
            if (elems[i] in BUILTIN_PROCS) {
                throw new Error(`${String(elems[i])}: cannot shadow builtin procedure`)
            }
        }

        // add in a begin block if theres more than one op
        const bodyAST = argCount === 2 ? expr[2] : [OP_BEGIN, ...expr.slice(2)];

        return new Closure(elems, bodyAST, scope)   
    }),
    [OP_LET]: new SpecialForm((vm, argCount, expr, scope, cs) => {
        // OP_LET is special in that it gets compiled down to a lambda in the end

        // normal let: (let ((var expr) ...) body1 body2 ...)
        if (argCount < 2) throw new Error(`let must be in format ["let", [[var expr]...], body...] but only have ${argCount} arguments`);

        const bindings = expr[1];
        if (!Array.isArray(bindings) && !(bindings instanceof Cons) && bindings !== null) {
            throw new Error("let arg 1 must be a list of form [[var expr]...]");
        }

        const body = expr.slice(2);
        const params: symbol[] = [];
        const exprs: any[] = [];

        if (bindings !== null) {
            for (const binding of bindings) {
                let sym, val;
                if (Array.isArray(binding)) {
                    if (binding.length != 2) {
                        throw new Error(`let binding \`${binding}\` must be a list of form [var expr] but only have list of length ${binding.length}`);
                    }
                    sym = binding[0];
                    val = binding[1];
                } else if (binding instanceof Cons) {
                    if (binding.length != 2) {
                        throw new Error(`let binding \`${binding}\` must be a list of form [var expr] but only have list of length ${binding.length}`);
                    }

                    sym = binding.head;
                    val = binding.tail?.head; 
                } else {
                    throw new Error(`let binding \`${binding}\` must be a list of form [var expr]`);
                }

                if (typeof sym !== "symbol") throw new Error("let binding name must be a symbol");
                
                params.push(sym);
                exprs.push(val);
            }
        }

        // rewrite to lambda [(let ((var expr) ...) body1 body2 ...) => ((lambda (var...) body1 body2...) expr...)]
        cs.expr = [[OP_LAMBDA, params, ...body], ...exprs];
        return SPECIAL_FORM_TCO_TRIGGER; 
    }),
    [OP_QUOTE]: new SpecialForm((vm, argCount, expr, scope, cs) => {
        if(argCount != 1) {
            throw new Error(`quote must be in format ["quote", expr] but have ${argCount} arguments`)
        }

        return expr[1];
    }),
    [OP_IF]: new SpecialForm((vm, argCount, expr, scope, cs) => {
        if(argCount != 3) {
            throw new Error(`if condition must be in format ["if", condition, true_expr, false_expr] but only have ${argCount} arguments`)
        }

        const cond = vm.evalinner(expr[1], scope); 
        
        // Branches are in tail position
        cs.expr = vm.isTruthy(cond) ? expr[2] : expr[3];
        return SPECIAL_FORM_TCO_TRIGGER;
    }),
    [OP_COND]: new SpecialForm((vm, argCount, expr, scope, cs) => {
        if (argCount === 0) throw new Error("cond requires at least one clause");
        
        let tailExpr = null;
        let hasMatch = false;

        for (let i = 0; i < argCount; i++) {
            const clause = expr[i + 1];
            
            if (!Array.isArray(clause) || clause.length !== 2) {
                throw new Error(`cond clause must be a list of exactly 2 elements: [condition, expr]`);
            }
            
            const condition = clause[0];
            const resultExpr = clause[1];
            
            // Check if it's the 'else' fallback, or if the condition evaluates to truthy. if so, we have a match
            // to tail-call on
            if (condition === OP_ELSE || vm.isTruthy(vm.evalinner(condition, scope))) {
                tailExpr = resultExpr;
                hasMatch = true;
                break;
            }
        }

        // If a branch matched, tail-call it
        if (hasMatch) {
            cs.expr = tailExpr;
            return SPECIAL_FORM_TCO_TRIGGER; 
        }
        
        // If nothing matches (meaning none of the if clauses resolved nor did the 'else'), return void
        return undefined; 
    }),
    [OP_AND]: new SpecialForm((vm, argCount, expr, scope, cs) => {
        if (argCount === 0) return true; 
        for (let i = 0; i < argCount - 1; i++) {
            const val = vm.evalinner(expr[i+1], scope)
            if(!vm.isTruthy(val)) return val
        }
            
        // Last expression is in tail position
        cs.expr = expr[argCount];
        return SPECIAL_FORM_TCO_TRIGGER;
    }),
    [OP_OR]: new SpecialForm((vm, argCount, expr, scope, cs) => {
        if (argCount === 0) return false;
        for (let i = 0; i < argCount - 1; i++) {
            const val = vm.evalinner(expr[i+1], scope)
            if (vm.isTruthy(val)) return val;
        }

        // Last expression is in tail position
        cs.expr = expr[argCount];
        return SPECIAL_FORM_TCO_TRIGGER;
    }),
    // @ts-ignore
    __proto__: null
}

export class Anima {
    disableLambda: boolean
    disableDefine: boolean
    maxSteps: number; // 0 to disable

    #currExprState = { expr: null as any };

    constructor(opts?: {disableLambda?: boolean, disableDefine?: boolean, maxSteps?: number}) {
        this.disableLambda = opts?.disableLambda || false
        this.disableDefine = opts?.disableDefine || false
        this.maxSteps = opts?.maxSteps || 0
    }

    public evaluate(expr: any, rawData: Record<string, any>): any {
        const globalScope = new AnimaScope(rawData, null, {steps: 0})
        const executionScope = globalScope.nest(); // Any "define" calls now write to this temporary scope
        return this.evalinner(expr, executionScope);
    }

    /** Returns if a value is truthy or not */
    isTruthy(val: any): boolean {
        return val !== false && val !== null && val !== undefined;
    }   

    // @internal
    isDeepEqual(a: any, b: any): boolean {
        // If simple eqv? logic works, return true as no more work needed
        if (Object.is(a, b)) return true;

        // Lists
        const aIsList = a instanceof Cons || Array.isArray(a);
        const bIsList = b instanceof Cons || Array.isArray(b);

        if (aIsList && bIsList) {
            const len = a.length;
            if (len !== b.length) return false;
            if (len === 0) return true;

            const aIsCons = a instanceof Cons;
            const bIsCons = b instanceof Cons;

            if (aIsCons && bIsCons) {
                let currA = a as Cons;
                let currB = b as Cons;
                for (let i = 0; i < len; i++) {
                    if (!this.isDeepEqual(currA.head, currB.head)) return false;
                    currA = currA.tail as Cons;
                    currB = currB.tail as Cons;
                }
                if (!this.isDeepEqual(currA, currB)) return false;
            } else if (!aIsCons && !bIsCons) {
                const arrA = a as any[];
                const arrB = b as any[];
                for (let i = 0; i < len; i++) {
                    if (!this.isDeepEqual(arrA[i], arrB[i])) return false;
                }
            } else if (aIsCons && !bIsCons) {
                let currA = a as Cons;
                const arrB = b as any[];
                for (let i = 0; i < len; i++) {
                    if (!this.isDeepEqual(currA.head, arrB[i])) return false;
                    currA = currA.tail as Cons;
                }
                if (currA !== null) return false;
            } else { // !aIsCons && bIsCons
                const arrA = a as any[];
                let currB = b as Cons;
                for (let i = 0; i < len; i++) {
                    if (!this.isDeepEqual(arrA[i], currB.head)) return false;
                    currB = currB.tail as Cons;
                }
                if (currB !== null) return false;
            }

            return true;
        }
    
        // Closures/other types
        return false;
    }

    // TCO stuff made with help of Gemini
    // @internal
    evalinner(initialExpr: any, initialScope: AnimaScope): any {
        let expr = initialExpr;
        let scope = initialScope;

        while (true) {
            scope.state.steps++;
            if (this.maxSteps && scope.state.steps > this.maxSteps) {
                throw new Error(`Execution Limits Exceeded: Script ran for more than ${this.maxSteps} cycles.`);
            }

            if (typeof expr === "string") return expr; // Strings evaluate to themselves
            if (typeof expr === "symbol") {
                return BUILTIN_PROCS[expr] || scope.get(expr);
            }

            if (!Array.isArray(expr)) return expr; // If not an array (boolean etc), it evaluates to the expression itself
            if (expr.length === 0) return null; // An empty array evaluates to null

            const operator = expr[0];
            const argCount = expr.length-1

            if (typeof operator === "symbol" && operator in SPECIAL_FORM_PROCS) {
                this.#currExprState.expr = expr;
                
                const result = SPECIAL_FORM_PROCS[operator].cb(this, argCount, expr, scope, this.#currExprState);
                
                if (result === SPECIAL_FORM_TCO_TRIGGER) {
                    expr = this.#currExprState.expr;
                    continue; 
                }
                
                return result;
            }

            let proc;
            
            // FAST PATH: if symbol
            if (typeof expr[0] === "symbol") {
                proc = BUILTIN_PROCS[expr[0]] || scope.get(expr[0])
            } else {
                // SLOW PATH: Dynamically computed procedures need to be eval'd explicitly 
                proc = this.evalinner(expr[0], scope);
            }
            
            // Handle builtins by directly passsing VM+expr+scope+computed argcount
            if (proc instanceof Builtin) {
                return proc.cb(this, argCount, expr, scope);
            }

            // Anima procedure
            if (proc instanceof Closure) {
                if (argCount != proc.params.length) {
                    throw new Error(`Attempted to call a procedure taking ${proc.params.length} arguments with ${argCount} arguments`);
                }
                
                const callargs = new Array(argCount)
                for (let i = 0; i < argCount; i++) {
                    callargs[i] = this.evalinner(expr[i+1], scope);
                }
                const callscope = proc.scope.nest();
                
                // bind args
                for (let i = 0; i < proc.params.length; i++) {
                    callscope.define(proc.params[i], callargs[i]);
                }

                // tail-call procedure body and newly bound callscope to avoid allocing new stack frame
                expr = proc.body;
                scope = callscope;
                continue;
            }
            throw new Error(`Unknown operator or attempted to call a non-procedure: ${String(operator)}`);
        }
    }

    // @internal
    //
    // Does not apply TCO
    execproc(proc: any, args: any[], scope: AnimaScope) {
        const argCount = args.length
        
        if (proc instanceof Builtin) {
            return proc.cb(this, argCount, [proc, ...args], scope);
        }

        if (proc instanceof Closure) {
            if (argCount != proc.params.length) {
                throw new Error(`Attempted to call a procedure taking ${proc.params.length} arguments with ${argCount} arguments`);
            }
            
            const callscope = proc.scope.nest();
            
            // bind args
            for (let i = 0; i < proc.params.length; i++) {
                callscope.define(proc.params[i], args[i]);
            }

            return this.evalinner(proc.body, callscope)
        }
    }
}


export class ASPTokenError extends Error {
    pos: number;
    curtok?: string;
    constructor(message: string, pos: number, curtok?: string) {
        super(message);
        this.name = 'ASPTokenError';
        this.pos = pos
        this.curtok = curtok
    }
}

export class ASPParseError extends Error {
    pos?: number;
    curtok?: string;
    constructor(message: string, pos?: number, curtok?: string) {
        super(message);
        this.name = 'ASPParseError';
        this.pos = pos
        this.curtok = curtok
    }
}

const ASP_SPECIAL_TOKENS = new Set(['(', ')', '[', ']', ';', '"', "'"])
export class ASP {    
    #str: string;
    #currPos: number;
    constructor(str: string) {
        this.#str = str
        this.#currPos = 0
    }

    /** Look at the current character without moving forward */
    private peek(): string {
        return this.#str[this.#currPos] || "";
    }

    /** Consume the current character and move forward */
    private advance(): string {
        return this.#str[this.#currPos++] || "";
    }

    /** are we done yet? */
    private isEOF(): boolean {
        return this.#currPos >= this.#str.length;
    }

    /** skip over trivia (like whitespace,comments etc.) */
    private skipTrivia(): void {
        while (!this.isEOF()) {
            const char = this.peek();
    
            // Drop whitespace
            if (/\s/.test(char)) {
                this.advance();
            } else if (char === ';') {
                // If we see a comment, consume everything until a newline
                while (!this.isEOF() && this.peek() !== '\n') {
                    this.advance();
                }
            } else {
                // We're done
                break; 
            }
        }    
    }
    
    /** Tokenize the input into a list of tokens to then parse */
    private tokenize(): string[] {
        const tokens: string[] = [];

        while (!this.isEOF()) {
            this.skipTrivia();
            if (this.isEOF()) break;
            const char = this.peek();

            // Lists
            if (char === '(' || char === ')' || char === '[' || char === ']') {
                tokens.push(this.advance());
                continue;
            }

            // Quote/'reader' has similar behavior to lists
            if (char === "'") {
                tokens.push(this.advance());
                continue;
            }

            // String literals
            if (char === '"') {
                let strToken = this.advance(); // Open "
                
                while (!this.isEOF() && this.peek() !== '"') {
                    if (this.peek() === '\\') {
                        strToken += this.advance(); // consume the slash (we will then consume the character in the general strToken advancer)
                    }
                    strToken += this.advance(); // consume the character
                }
                
                if (this.peek() === '"') {
                    strToken += this.advance(); // Consume the closing quote
                } else {
                    throw new ASPTokenError(`Unterminated string literal`, this.#currPos, strToken);
                }
                
                tokens.push(strToken);
                continue;
            }

            // All other literals (numbers, symbols, booleans etc)
            let atom = "";
            while (
                !this.isEOF() && 
                !/\s/.test(this.peek()) && 
                !ASP_SPECIAL_TOKENS.has(this.peek())
            ) {
                atom += this.advance();
            }
            tokens.push(atom);
        }

        return tokens
    }

    /** Parses tokenized string and builds the final expr */
    public parse(): any {
        const tokens = this.tokenize();
        let current = 0;

        const walk = (): any => {
            if (current >= tokens.length) {
                throw new ASPParseError(`Unexpected end of input: Missing closing bracket.`, current);
            }

            let token = tokens[current];

            // Quote
            if (token === "'") {
                current++; // Skip the quote
                if (current >= tokens.length) {
                    throw new ASPParseError("Unexpected end of input: Missing expression after '", current);
                }
                const nextExpr = walk(); // Parse the next expr after the quote
                return [OP_QUOTE, nextExpr];  // Wrap in quote builtin proc
            }

            // Lists
            if (token === '(' || token === '[') {
                const expectedClose = token === '(' ? ')' : ']';
                current++; 
                const lst: any[] = [];
                
                while (tokens[current] !== expectedClose) {
                    if (current >= tokens.length || tokens[current] === ')' || tokens[current] === ']') {
                        throw new ASPParseError(`Mismatched or missing closing bracket for '${token}'`, current);
                    }
                    lst.push(walk());
                }
                
                current++; 
                return lst;
            }

            // Stray closing brackets are not allowed
            if (token === ')' || token === ']') {
                throw new ASPParseError("Unexpected closing bracket", current, token);
            }

            // All other literals (numbers, symbols, booleans etc)
            current++; // consume current token

            // Booleans+null (which is empty list)
            if (token === '#t') return true;
            if (token === '#f') return false;
            if (token === 'null') return null;

            // Numbers
            const num = Number(token);
            if (!Number.isNaN(num)) return num;

            // Strings must be (un?)escaped
            if (token.startsWith('"') && token.endsWith('"')) {
                try {
                    // HACK: JSON.parse should parse this correctly
                    const string = JSON.parse(token); 
                    return string;
                } catch (e) {
                    throw new ASPParseError(`String parse failed (${e})`, current, token)
                }
            }

            // Symbol
            return Symbol.for(token);
        };

        const exprs = []
        while (current < tokens.length) {
            exprs.push(walk());
        }
        if (exprs.length == 0) return null
        if (exprs.length == 1) return exprs[0]

        // Translate to begin
        return [OP_BEGIN, ...exprs];
    }
}

export class ASTStringifier {
    constructor() {}
    public stringify(ast: any): string {
        // Booleans+null+number
        if (ast === null) return "null";
        if (typeof ast === "number") {
            return String(ast);
        } else if (typeof ast === "boolean") {
            return ast ? "#t" : "#f"
        }

        // String
        if (typeof ast === "string") {
            return JSON.stringify(ast);
        }

        // Symbol
        if (typeof ast === "symbol") {
            return /*Symbol.keyFor(ast)*/ ast.description || ast.toString();
        }

        // Lists
        if (Array.isArray(ast)) {
            const lst = new Array(ast.length)
            for(let i = 0; i < ast.length; i++) {
                lst[i] = this.stringify(ast[i]);
            }
            return `(${lst.join(" ")})`;
        }

        // Cons
        if (ast instanceof Cons) {
            const parts: string[] = [];
            let current: any = ast;

            while (current !== null) {
                if (current instanceof Cons) {
                    parts.push(this.stringify(current.head));
                    current = current.tail;
                } else {
                    // Improper list/pair
                    parts.push(".");
                    parts.push(this.stringify(current));
                    break;
                }
            }
            return `(${parts.join(" ")})`;
        }

        // Procs
        if (ast instanceof Builtin || ast instanceof Closure) {
            return `<procedure>`;
        }

        // Undefined
        if (ast === undefined) return `<#void>`

        throw new Error(`Cannot stringify unknown AST node: ${JSON.stringify(ast)}`);
    }
}