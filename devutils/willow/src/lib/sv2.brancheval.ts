class FormBranchEvaluatorScope {
    #data: Record<string, any>; // from svelte $state etc
    #outer: FormBranchEvaluatorScope | null;

    constructor(data: Record<string, any>, outer: FormBranchEvaluatorScope | null) {
        this.#data = data
        this.#outer = outer
    }

    nest(): FormBranchEvaluatorScope {
        // Nested scopes don't need to be reactive
        return new FormBranchEvaluatorScope(Object.create(null), this);
    }

    get(key: string): any {
        let scope: FormBranchEvaluatorScope | null = this
        while(scope) {
            if (Object.prototype.hasOwnProperty.call(scope.#data, key)) {
                return scope.#data[key];
            }            
            scope = scope.#outer
        }
        return null
    }

    set(key: string, value: any) {
        this.#data[key] = value;
    }
} 

class Closure {
    params: string[];
    body: any;
    scope: FormBranchEvaluatorScope;

    constructor(params: string[], body: any, scope: FormBranchEvaluatorScope) {
        this.params = params
        this.body = body
        this.scope = scope
    }
}

class JSClosureState {
    vmexpr: any; // the current inst the vm is evaluating
    vmscope: FormBranchEvaluatorScope;

    constructor(vmexpr: any, vmscope: FormBranchEvaluatorScope) {
        this.vmexpr = vmexpr
        this.vmscope = vmscope
    }
}

abstract class JSClosure {
    /* The bound scope */
    scope: FormBranchEvaluatorScope;
    constructor(scope: FormBranchEvaluatorScope) {
        this.scope = scope
    }

    // If the VM is to continue on executing (tailcalls to be evaluated on next vm loop), 
    // then return [true, null], otherwise, return [false, val]
    //
    // As an example:
    //
    // class NativeEval extends JSClosure {
    //      call(state: JSClosureState, callargs: any[]): [boolean, any] {
    //          // We want the VM to evaluate whatever AST was passed in
    //          state.vmexpr = callargs[0]; 
    //    
    //          // Return true to tell the VM: "I mutated the state, continue the loop!"
    //          return [true, null]; 
    //      } 
    //  }
    abstract call(state: JSClosureState, callargs: any[]): [boolean, any]
}

export class Anima {
    constructor() {

    }

    public computeBranch(expr: any, rawData: Record<string, any>): any {
        const globalScope = new FormBranchEvaluatorScope(rawData, null)
        const executionScope = globalScope.nest(); // Any "define" calls now write to this temporary scope
        return this.evaluate(expr, executionScope);
    }

    private isTruthy(val: any): boolean {
        return val !== false && val !== null && val !== undefined;
    }

    // TCO stuff made with help of Gemini
    private evaluate(initialExpr: any, initialScope: FormBranchEvaluatorScope): any {
        let expr = initialExpr;
        let scope = initialScope;

        while (true) {
            if (typeof expr === "string") {
                if (expr.startsWith("'")) {
                    return expr.slice(1);
                } else {
                    return scope.get(expr); // FIXED!
                }
            }            
            if (!Array.isArray(expr)) return expr; // If not an array (boolean etc), it evaluates to the expression itself
            if (expr.length === 0) return null; // An empty array evaluates to null

            const [operator, ...args] = expr;
            switch (operator) {
                case "get": 
                    if(args.length != 1) {
                        throw new Error(`get must be in format ["get", expr] but only have ${args.length} arguments`)
                    }

                    return scope.get(args[0]);

                case "define": {
                    if(args.length != 2) {
                        throw new Error(`define must be in format ["define", varname, arg] but only have ${args.length} arguments`)
                    }
                    if(typeof args[0] != "string") {
                        throw new Error(`define: argument 1 must be a string`)
                    }
                    const val = this.evaluate(args[1], scope);
                    scope.set(args[0], val);
                    return val;
                }

                case "do":
                    // Executes a sequence, tail-calls the very last expression.
                    // e.g., ["do", ["def", "x", 1], ["+", ["get", "x"], 1]]
                    if(args.length == 0) {
                        throw new Error(`do must be in format ["do", ...] but only have ${args.length} arguments`)
                    }

                    for (let i = 0; i < args.length - 1; i++) {
                        this.evaluate(args[i], scope);
                    }

                    expr = args[args.length - 1];
                    continue;

                case "lambda":
                    if(args.length != 2) {
                        throw new Error(`lambda must be in format ["lambda", [bind-args...], arg2] but only have ${args.length} arguments`)
                    }

                    return new Closure(args[0], args[1], scope)   

                // Type checkers
                case "type?": {
                    if(args.length != 1) {
                        throw new Error(`type? must be in format ["type?", expr] but only have ${args.length} arguments`)
                    }

                    const resolvedValue = this.evaluate(args[0], scope);
                    if (resolvedValue === null) return "null";
                    switch(typeof resolvedValue) {
                        case "string": return "string"
                        case "number": return "number"
                        case "boolean": return "boolean"
                        case "undefined": return "null"
                        default: {
                            if(resolvedValue instanceof Closure) return "procedure"
                            if(resolvedValue instanceof JSClosure) return "js-procedure"
                            return "unknown" // to allow consistency across all js engines/custom sv2 impls etc.
                        }
                    }
                }

                case "list":
                    return args.map(arg => this.evaluate(arg, scope));

                case "quote":
                    return args[0];

                case "length": {
                    if(args.length != 1) {
                        throw new Error(`length must be in format ["length", expr] but only have ${args.length} arguments`)
                    }

                    const target = this.evaluate(args[0], scope);
                    return Array.isArray(target) ? target.length : (typeof target === "string" ? target.length : 0);
                }

                case "contains": {
                    if(args.length != 2) {
                        throw new Error(`contains must be in format ["contains", expr, contains_expr] but only have ${args.length} arguments`)
                    }

                    const list = this.evaluate(args[0], scope);
                    const item = this.evaluate(args[1], scope);
                    return Array.isArray(list) ? list.includes(item) : false;
                }
        
                // Control flow
                case "if": {
                    if(args.length != 3) {
                        throw new Error(`if condition must be in format ["if", condition, true_expr, false_expr] but only have ${args.length} arguments`)
                    }

                    const cond = this.evaluate(args[0], scope); 
                    
                    // Branches are in tail position
                    expr = this.isTruthy(cond) ? args[1] : args[2];
                    continue;
                }

                // Logic (all logic short circuits)
                case "==": return this.evaluate(args[0], scope) === this.evaluate(args[1], scope);
                case "!=": return this.evaluate(args[0], scope) !== this.evaluate(args[1], scope);
                case "and": { 
                    if (args.length === 0) return true; 
                    for (let i = 0; i < args.length - 1; i++) {
                        const val = this.evaluate(args[i], scope)
                        if(!this.isTruthy(val)) return val
                    }
                        
                    // Last expression is in tail position
                    expr = args[args.length - 1];
                    continue;
                }
                case "or": {
                    if (args.length === 0) return false;
                    for (let i = 0; i < args.length - 1; i++) {
                        const val = this.evaluate(args[i], scope)
                        if (this.isTruthy(val)) return val;
                    }

                    // Last expression is in tail position
                    expr = args[args.length - 1];
                    continue;
                }
                                
                // Math
                case ">": return this.evaluate(args[0], scope) > this.evaluate(args[1], scope);
                case "<": return this.evaluate(args[0], scope) < this.evaluate(args[1], scope);
                case ">=": return this.evaluate(args[0], scope) >= this.evaluate(args[1], scope);
                case "<=": return this.evaluate(args[0], scope) <= this.evaluate(args[1], scope);    
                case "+": return this.evaluate(args[0], scope) + this.evaluate(args[1], scope);
                case "-": return this.evaluate(args[0], scope) - this.evaluate(args[1], scope);
                case "*": return this.evaluate(args[0], scope) * this.evaluate(args[1], scope);
                case "/": return this.evaluate(args[0], scope) / this.evaluate(args[1], scope);
                case "%": return this.evaluate(args[0], scope) % this.evaluate(args[1], scope);
                
                default: {
                    // Procedure call if unknown
                    const proc = this.evaluate(operator, scope)
                    
                    // JS procedure call
                    if (proc instanceof JSClosure) {
                        const callargs = args.map(a => this.evaluate(a, scope));

                        // directly call closure, we also pass expr+scope to the function through JSClosureState
                        // to enable for JS functions to perform TCO optimization and view the currently executing
                        // closures scope
                        const vmstate = new JSClosureState(expr, scope)
                        const [tcoCont, retVal] = proc.call(vmstate, callargs)
                        if(tcoCont) {
                            expr = vmstate.vmexpr
                            scope = vmstate.vmscope
                            continue
                        }
                        return retVal
                    }

                    // Anima procedure
                    if (proc instanceof Closure) {
                        const callargs = args.map(a => this.evaluate(a, scope));
                        const callscope = proc.scope.nest();
                        if (callargs.length != proc.params.length) {
                            throw new Error(`Attempted to call a procedure taking ${proc.params.length} arguments with ${callargs.length} arguments`);
                        }
                        proc.params.forEach((paramName: string, idx: number) => {
                            callscope.set(paramName, callargs[idx]);
                        });

                        // tail-call (optimization) with procedure body and newly bound callscope to avoid allocing new stack frame (similar to Scheme)
                        expr = proc.body;
                        scope = callscope;
                        continue;
                    }
                    throw new Error(`Unknown operator or attempted to call a non-procedure: ${JSON.stringify(operator)}`);
                }
            }
        }
    }
}
