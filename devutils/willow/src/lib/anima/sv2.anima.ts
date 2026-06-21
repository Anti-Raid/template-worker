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
const OP_DEFINE = Symbol.for("define");
const OP_DO     = Symbol.for("do");
const OP_LAMBDA = Symbol.for("lambda");
const OP_IF     = Symbol.for("if");
const OP_COND   = Symbol.for("cond");
const OP_ELSE   = Symbol.for("else");
const OP_QUOTE  = Symbol.for("quote");

// List Operations
const OP_LIST     = Symbol.for("list");
const OP_CONS     = Symbol.for("cons")
const OP_CAR      = Symbol.for("car");
const OP_CDR      = Symbol.for("cdr");
const OP_LAST     = Symbol.for("last");
const OP_LENGTH   = Symbol.for("length");
const OP_EMPTY    = Symbol.for("empty?")
const OP_CONTAINS = Symbol.for("contains");

// Logic & Type Checking
const OP_AND      = Symbol.for("and");
const OP_OR       = Symbol.for("or");
const OP_NOT      = Symbol.for("not");
const OP_TYPE     = Symbol.for("type?");
const OP_EQ       = Symbol.for("=");
const OP_EQ_PTR1  = Symbol.for("eq?");
const OP_EQ_PTR2  = Symbol.for("eqv?");
const OP_EQ_DEEP1 = Symbol.for("equal?");
const OP_EQ_DEEP2 = Symbol.for("equals?");

// Math & Comparisons
const OP_LT     = Symbol.for("<");
const OP_GT     = Symbol.for(">");
const OP_LTE    = Symbol.for("<=");
const OP_GTE    = Symbol.for(">=");
const OP_ADD    = Symbol.for("+");
const OP_SUB    = Symbol.for("-");
const OP_MUL    = Symbol.for("*");
const OP_DIV    = Symbol.for("/");
const OP_MODULO = Symbol.for("modulo");

export const SPECIAL_FORMS = new Set([
    OP_DEFINE, 
    OP_QUOTE, 
    OP_LAMBDA, 
    OP_IF, 
    OP_COND, 
    OP_ELSE, 
    OP_AND, 
    OP_OR, 
    OP_DO
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

/** Builtin procedures */
export const BUILTIN_PROCS: Record<symbol, Builtin> = {
    [OP_ADD]: new Builtin((vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => {
        if (argCount < 2) throw new Error("+ requires at least 2 arguments");
        let result = vm.evalinner(expr[1], scope);
        if (typeof result !== "number") throw new Error("+ requires numbers");
        
        for(let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i+1], scope);
            if (typeof next !== "number") throw new Error("+ requires numbers");
            result += next; 
        }
        return result;
    }),
    [OP_SUB]: new Builtin((vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => {
        if (argCount < 2) throw new Error("- requires at least 2 arguments");
        let result = vm.evalinner(expr[1], scope);
        if (typeof result !== "number") throw new Error("- requires numbers");
        
        for(let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i+1], scope);
            if (typeof next !== "number") throw new Error("- requires numbers");
            result -= next; 
        }
        return result;
    }),
    [OP_MUL]: new Builtin((vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => {
        if (argCount < 2) throw new Error("* requires at least 2 arguments");
        let result = vm.evalinner(expr[1], scope);
        if (typeof result !== "number") throw new Error("* requires numbers");
        
        for(let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i+1], scope);
            if (typeof next !== "number") throw new Error("* requires numbers");
            result *= next; 
        }
        return result;
    }),
    [OP_DIV]: new Builtin((vm: Anima, argCount: number, expr: any[], scope: AnimaScope) => {
        if (argCount < 2) throw new Error("/ requires at least 2 arguments");
        let result = vm.evalinner(expr[1], scope);
        if (typeof result !== "number") throw new Error("/ requires numbers");
        
        for(let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i+1], scope);
            if (typeof next !== "number") throw new Error("/ requires numbers");
            result /= next; 
        }
        return result;
    }),
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
            throw new Error("cdr requires a list");
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
    [OP_NOT]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount != 1) throw new Error("not requires 1 argument");
        return !vm.isTruthy(vm.evalinner(expr[1], scope));
    }),
    [OP_EQ]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount < 2) throw new Error("= requires at least 2 arguments");
        let prev = vm.evalinner(expr[1], scope);
        if (typeof prev !== "number") throw new Error("= requires numbers");
        
        for (let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i+1], scope);
            if (typeof next !== "number") throw new Error("= requires numbers");
            
            if (prev !== next) return false;
            prev = next;
        }
        return true;
    }),    
    [OP_EQ_PTR1]: strictEqProc,
    [OP_EQ_PTR2]: strictEqProc,
    [OP_EQ_DEEP1]: deepEqProc,
    [OP_EQ_DEEP2]: deepEqProc,

    // comparison operators
    [OP_GT]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount < 2) throw new Error("> requires at least 2 arguments");
        let prev = vm.evalinner(expr[1], scope);
        if (typeof prev !== "number") throw new Error("> requires numbers");
        
        for (let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i+1], scope);
            if (typeof next !== "number") throw new Error("> requires numbers");
            
            if (!(prev > next)) return false;
            prev = next;
        }
        return true;
    }),
    
    [OP_LT]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount < 2) throw new Error("< requires at least 2 arguments");
        let prev = vm.evalinner(expr[1], scope);
        if (typeof prev !== "number") throw new Error("< requires numbers");
        
        for (let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i+1], scope);
            if (typeof next !== "number") throw new Error("< requires numbers");
            
            if (!(prev < next)) return false;
            prev = next;
        }
        return true;
    }),
    
    [OP_GTE]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount < 2) throw new Error(">= requires at least 2 arguments");
        let prev = vm.evalinner(expr[1], scope);
        if (typeof prev !== "number") throw new Error(">= requires numbers");
        
        for (let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i+1], scope);
            if (typeof next !== "number") throw new Error(">= requires numbers");
            
            if (!(prev >= next)) return false;
            prev = next;
        }
        return true;
    }),
    
    [OP_LTE]: new Builtin((vm, argCount, expr, scope) => {
        if (argCount < 2) throw new Error("<= requires at least 2 arguments");
        let prev = vm.evalinner(expr[1], scope);
        if (typeof prev !== "number") throw new Error("<= requires numbers");
        
        for (let i = 1; i < argCount; i++) {
            const next = vm.evalinner(expr[i+1], scope);
            if (typeof next !== "number") throw new Error("<= requires numbers");
            
            if (!(prev <= next)) return false;
            prev = next;
        }
        return true;
    }),
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
                return "unknown" // to allow consistency across all js engines/custom sv2 impls etc.
            }
        }
    }),
}

export class Anima {
    disableLambda: boolean
    disableDefine: boolean
    maxSteps: number; // 0 to disable
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
            switch (operator) {
                case OP_DEFINE: {
                    if (this.disableDefine) {
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

                        const val = this.evalinner(expr[2], scope);
                        scope.define(expr[1], val);
                        return val;
                    } else if (Array.isArray(expr[1])) {
                        // (define (func_name arg1 arg2) body_expr...)
                        if (expr[1].length === 0) throw new Error("define: missing function name");
                        const funcName = expr[1][0];
                        const params = expr[1].slice(1);
                        const body = expr.slice(2);
                        expr = [OP_DEFINE, funcName, [OP_LAMBDA, params, ...body]];
                        continue
                    } else if (expr[1] instanceof Cons) {
                        const funcName = expr[1].head;
                        const params = expr[1].tail;
                        if (!params) throw new Error("define: missing params");
                        const body = expr.slice(2);
                        expr = [OP_DEFINE, funcName, [OP_LAMBDA, params, ...body]];
                        continue
                    } else {
                        throw new Error(`${String(expr[1])}: expr[1] not symbol or list syntax`)
                    }
                }

                // Executes a sequence of expressions, last expr is tail-called
                case OP_DO: {
                    if(argCount == 0) {
                        throw new Error(`do must be in format ["do", ...] but have ${argCount} arguments`)
                    }

                    for (let i = 0; i < argCount - 1; i++) {
                        this.evalinner(expr[i+1], scope);
                    }

                    expr = expr[argCount];
                    continue;
                }

                case OP_LAMBDA: {
                    if(this.disableLambda) {
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

                    // add in a do block if theres more than one op
                    const bodyAST = argCount === 2 ? expr[2] : [OP_DO, ...expr.slice(2)];

                    return new Closure(elems, bodyAST, scope)   
                }

                case OP_QUOTE: {
                    if(argCount != 1) {
                        throw new Error(`quote must be in format ["quote", expr] but have ${argCount} arguments`)
                    }

                    return expr[1];
                }
        
                // Control flow
                case OP_IF: {
                    if(argCount != 3) {
                        throw new Error(`if condition must be in format ["if", condition, true_expr, false_expr] but only have ${argCount} arguments`)
                    }

                    const cond = this.evalinner(expr[1], scope); 
                    
                    // Branches are in tail position
                    expr = this.isTruthy(cond) ? expr[2] : expr[3];
                    continue;
                }

                case OP_COND: {
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
                        if (condition === OP_ELSE || this.isTruthy(this.evalinner(condition, scope))) {
                            tailExpr = resultExpr;
                            hasMatch = true;
                            break;
                        }
                    }

                    // If a branch matched, tail-call it
                    if (hasMatch) {
                        expr = tailExpr;
                        continue; 
                    }
                    
                    // If nothing matches (meaning none of the if clauses resolved nor did the 'else'), return null
                    return null; 
                }

                case OP_AND: { 
                    if (argCount === 0) return true; 
                    for (let i = 0; i < argCount - 1; i++) {
                        const val = this.evalinner(expr[i+1], scope)
                        if(!this.isTruthy(val)) return val
                    }
                        
                    // Last expression is in tail position
                    expr = expr[argCount];
                    continue;
                }

                case OP_OR: {
                    if (argCount === 0) return false;
                    for (let i = 0; i < argCount - 1; i++) {
                        const val = this.evalinner(expr[i+1], scope)
                        if (this.isTruthy(val)) return val;
                    }

                    // Last expression is in tail position
                    expr = expr[argCount];
                    continue;
                }
                                
                default: {
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

            // Booleans+null
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

        // Translate to do
        return [OP_DO, ...exprs];
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