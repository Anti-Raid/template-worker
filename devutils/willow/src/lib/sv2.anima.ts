export class MissingVarError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'MissingVarError';
    }
}

class AnimaScope {
    #data: Record<string, any>; // from svelte $state etc
    #outer: AnimaScope | null;

    constructor(data: Record<string, any>, outer: AnimaScope | null) {
        this.#data = data
        this.#outer = outer
    }

    nest(): AnimaScope {
        // Nested scopes don't need to be reactive
        return new AnimaScope(Object.create(null), this);
    }

    get(key: string): any {
        let scope: AnimaScope | null = this
        while(scope) {
            if (Object.prototype.hasOwnProperty.call(scope.#data, key)) {
                return scope.#data[key];
            }            
            scope = scope.#outer
        }
        throw new MissingVarError(`Variable '${key}' is not defined in the current scope.`);
    }

    set(key: string, value: any) {
        if (this.#outer === null) throw new Error("Cannot set key on global scope")
        this.#data[key] = value;
    }
} 

class Closure {
    params: string[];
    body: any;
    scope: AnimaScope;

    constructor(params: string[], body: any, scope: AnimaScope) {
        this.params = params
        this.body = body
        this.scope = scope
    }
}

class JSClosureState {
    vmexpr: any; // the current inst the vm is evaluating
    vmscope: AnimaScope;

    constructor(vmexpr: any, vmscope: AnimaScope) {
        this.vmexpr = vmexpr
        this.vmscope = vmscope
    }
}

abstract class JSClosure {
    /* The bound scope */
    scope: AnimaScope;
    constructor(scope: AnimaScope) {
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

    public evaluate(expr: any, rawData: Record<string, any>): any {
        const globalScope = new AnimaScope(rawData, null)
        const executionScope = globalScope.nest(); // Any "define" calls now write to this temporary scope
        return this.evalinner(expr, executionScope);
    }

    private isTruthy(val: any): boolean {
        return val !== false && val !== null && val !== undefined;
    }

    private preparelistop(op: string, args: any[], scope: AnimaScope, minlen: number) {
        if (args.length != 1) {
            throw new Error(`${op} must be in format ["${op}", expr] but only have ${args.length} arguments`)
        }
        const val = this.evalinner(args[0], scope)
        if (!Array.isArray(val)) {
            throw new Error(`${op} expr must evaluate to a list`)
        } else if (val.length < minlen) {
            throw new Error(`${op} list has ${val.length} elements, but must have at least ${minlen} elements`)
        }
        return val
    }

    // TCO stuff made with help of Gemini
    private evalinner(initialExpr: any, initialScope: AnimaScope): any {
        let expr = initialExpr;
        let scope = initialScope;

        while (true) {
            if (typeof expr === "string") {
                if (expr.startsWith("'")) {
                    // equivalent to [quote stringhere]
                    return expr.slice(1);
                } else {
                    return scope.get(expr);
                }
            }            
            if (!Array.isArray(expr)) return expr; // If not an array (boolean etc), it evaluates to the expression itself
            if (expr.length === 0) return []; // An empty array evaluates to []
            
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
                    const val = this.evalinner(args[1], scope);
                    scope.set(args[0], val);
                    return val;
                }

                // Executes a sequence of expressions, last expr is tail-called
                case "do":
                    if(args.length == 0) {
                        throw new Error(`do must be in format ["do", ...] but only have ${args.length} arguments`)
                    }

                    for (let i = 0; i < args.length - 1; i++) {
                        this.evalinner(args[i], scope);
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

                    const resolvedValue = this.evalinner(args[0], scope);
                    if (resolvedValue === null) return "null";
                    switch(typeof resolvedValue) {
                        case "string": return "string"
                        case "number": return "number"
                        case "boolean": return "boolean"
                        case "undefined": return "null"
                        default: {
                            if(resolvedValue instanceof Closure) return "procedure"
                            if(resolvedValue instanceof JSClosure) return "js-procedure"
                            if(Array.isArray(resolvedValue)) return "list"
                            return "unknown" // to allow consistency across all js engines/custom sv2 impls etc.
                        }
                    }
                }

                case "list":
                    return args.map(arg => this.evalinner(arg, scope));
                case "car": {
                    const val = this.preparelistop("car", args, scope, 1)
                    return val[0]
                }
                case "cdr": {
                    const val = this.preparelistop("cdr", args, scope, 1)
                    return val.slice(1)
                }
                case "last": {
                    const val = this.preparelistop("last", args, scope, 1);
                    return val[val.length - 1];
                }
                case "quote":
                    if(args.length != 1) {
                        throw new Error(`quote must be in format ["quote", expr] but have ${args.length} arguments`)
                    }

                    return args[0];

                case "length": {
                    if(args.length != 1) {
                        throw new Error(`length must be in format ["length", expr] but only have ${args.length} arguments`)
                    }

                    const target = this.evalinner(args[0], scope);
                    return Array.isArray(target) ? target.length : (typeof target === "string" ? target.length : 0);
                }

                case "contains": {
                    if(args.length != 2) {
                        throw new Error(`contains must be in format ["contains", expr, contains_expr] but only have ${args.length} arguments`)
                    }

                    const list = this.evalinner(args[0], scope);
                    const item = this.evalinner(args[1], scope);
                    return Array.isArray(list) ? list.includes(item) : false;
                }
        
                // Control flow
                case "if": {
                    if(args.length != 3) {
                        throw new Error(`if condition must be in format ["if", condition, true_expr, false_expr] but only have ${args.length} arguments`)
                    }

                    const cond = this.evalinner(args[0], scope); 
                    
                    // Branches are in tail position
                    expr = this.isTruthy(cond) ? args[1] : args[2];
                    continue;
                }

                // Logic (all logic short circuits)
                case "==": return this.evalinner(args[0], scope) === this.evalinner(args[1], scope);
                case "!=": return this.evalinner(args[0], scope) !== this.evalinner(args[1], scope);
                case "and": { 
                    if (args.length === 0) return true; 
                    for (let i = 0; i < args.length - 1; i++) {
                        const val = this.evalinner(args[i], scope)
                        if(!this.isTruthy(val)) return val
                    }
                        
                    // Last expression is in tail position
                    expr = args[args.length - 1];
                    continue;
                }
                case "or": {
                    if (args.length === 0) return false;
                    for (let i = 0; i < args.length - 1; i++) {
                        const val = this.evalinner(args[i], scope)
                        if (this.isTruthy(val)) return val;
                    }

                    // Last expression is in tail position
                    expr = args[args.length - 1];
                    continue;
                }
                                
                // Math
                case ">": return this.evalinner(args[0], scope) > this.evalinner(args[1], scope);
                case "<": return this.evalinner(args[0], scope) < this.evalinner(args[1], scope);
                case ">=": return this.evalinner(args[0], scope) >= this.evalinner(args[1], scope);
                case "<=": return this.evalinner(args[0], scope) <= this.evalinner(args[1], scope);    
                case "+": return this.evalinner(args[0], scope) + this.evalinner(args[1], scope);
                case "-": return this.evalinner(args[0], scope) - this.evalinner(args[1], scope);
                case "*": return this.evalinner(args[0], scope) * this.evalinner(args[1], scope);
                case "/": return this.evalinner(args[0], scope) / this.evalinner(args[1], scope);
                case "%": return this.evalinner(args[0], scope) % this.evalinner(args[1], scope);
                
                default: {
                    // Procedure call if unknown
                    const proc = this.evalinner(operator, scope)
                    
                    // JS procedure call
                    if (proc instanceof JSClosure) {
                        const callargs = args.map(a => this.evalinner(a, scope));

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
                        const callargs = args.map(a => this.evalinner(a, scope));
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
                const nextExpr = walk(); // Parse the next expr after the quote
                return ["quote", nextExpr];  // Wrap in quote builtin proc
            }

            // Lists
            if (token === '(' || token === '[') {
                current++; // Skip the opening bracket
                const lst: any[] = [];
                
                // Keep walking recursively until we reach a closing bracket
                while (tokens[current] !== ')' && tokens[current] !== ']') {
                    if (current >= tokens.length) {
                        throw new ASPParseError(`Unexpected end of input: Missing closing bracket for '${token}'`, current);
                    }
                    lst.push(walk());
                }
                
                current++; // Skip the closing bracket
                return lst;
            }

            // Stray closing brackets are not allowed
            if (token === ')' || token === ']') {
                throw new ASPParseError("Unexpected closing bracket", current, token);
            }

            // All other literals (numbers, symbols, booleans etc)
            current++; // consume current token

            // Booleans+null
            if (token === 'true') return true;
            if (token === 'false') return false;
            if (token === 'null') return null;

            // Numbers
            const num = Number(token);
            if (!Number.isNaN(num)) return num;

            // Strings must be (un?)escaped and quote'd
            if (token.startsWith('"') && token.endsWith('"')) {
                try {
                    // HACK: JSON.parse should parse this correctly
                    const string = JSON.parse(token); 
                    return ["quote", string];
                } catch (e) {
                    throw new ASPParseError(`String parse failed (${e})`, current, token)
                }
            }

            // Symbol
            return token;
        };

        const exprs = []
        while (current < tokens.length) {
            exprs.push(walk());
        }
        if (exprs.length == 0) return null
        if (exprs.length == 1) return exprs[0]

        // Translate to do
        return ["do", ...exprs];
    }
}

export class ASTStringifier {
    public static stringify(ast: any): string {
        // Booleans+null+number
        if (ast === null) return "null";
        if (typeof ast === "number" || typeof ast === "boolean") {
            return String(ast);
        }

        // Symbol
        if (typeof ast === "string") {
            return ast;
        }

        // Lists
        if (Array.isArray(ast)) {
            if (ast[0] === "quote" && ast.length === 2) {
                const inner = ast[1];
                
                // Quoted list
                if (Array.isArray(inner)) {
                    return `'(${this.stringify(inner)})`;
                }
                
                return JSON.stringify(inner);
            }

            return `(${ast.map(ASTStringifier.stringify).join(" ")})`;
        }

        throw new Error(`Cannot stringify unknown AST node: ${JSON.stringify(ast)}`);
    }
}