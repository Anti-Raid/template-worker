import { Cons } from "./list";

/** Returns if a value is truthy or not */
export const isTruthy = (val: any): boolean => {
    return val !== false
}   

// @internal
export const isDeepEqual = (a: any, b: any): boolean => {
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
                if (!isDeepEqual(currA.head, currB.head)) return false;
                currA = currA.tail as Cons;
                currB = currB.tail as Cons;
            }
            if (!isDeepEqual(currA, currB)) return false;
        } else if (!aIsCons && !bIsCons) {
            const arrA = a as any[];
            const arrB = b as any[];
            for (let i = 0; i < len; i++) {
                if (!isDeepEqual(arrA[i], arrB[i])) return false;
            }
        } else if (aIsCons && !bIsCons) {
            let currA = a as Cons;
            const arrB = b as any[];
            for (let i = 0; i < len; i++) {
                if (!isDeepEqual(currA.head, arrB[i])) return false;
                currA = currA.tail as Cons;
            }
            if (currA !== null) return false;
        } else { // !aIsCons && bIsCons
            const arrA = a as any[];
            let currB = b as Cons;
            for (let i = 0; i < len; i++) {
                if (!isDeepEqual(arrA[i], currB.head)) return false;
                currB = currB.tail as Cons;
            }
            if (currB !== null) return false;
        }

        return true;
    }

    // Closures/other types
    return false;
}

export class MissingVarError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'MissingVarError';
    }
}

export class AnimaScope {
    #data: Record<string | symbol, any>;
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
        let scope: AnimaScope | null = this
        while(scope) {
            if (key in scope.#data) {
                return scope.#data[key];
            }
            
            // Outer scope is the only bit that can have string symbols, so check for that too
            if (scope.#outer === null && key.description) {
                if (Object.hasOwn(scope.#data, key.description)) {
                    return scope.#data[key.description];
                }
            }
            
            scope = scope.#outer
        }
        throw new MissingVarError(`Variable '${String(key)}' is not defined in the current scope.`);
    }

    define(key: symbol, value: any) {
        if (this.#outer === null) throw new Error("Cannot define key on global scope")
        this.#data[key] = value;
    }

    // set!
    set(key: symbol, value: any): any {
        let scope: AnimaScope | null = this
        while(scope) {
            if (scope.#outer === null) throw new Error("Cannot set! key on global scope")
            if (key in scope.#data) {
                scope.#data[key] = value;
                return
            }
            
            scope = scope.#outer
        }
        throw new MissingVarError(`Variable '${String(key)}' is not defined in the current scope.`);
    }
}

// Special Forms
export const OP_DEFINE = Symbol.for("define");
export const OP_SET    = Symbol.for("set!")
export const OP_BEGIN     = Symbol.for("begin");
export const OP_LAMBDA = Symbol.for("lambda");
export const OP_LET    = Symbol.for("let");
export const OP_LETSTAR = Symbol.for("let*")
export const OP_LETREC = Symbol.for("letrec")
export const OP_IF     = Symbol.for("if");
export const OP_COND   = Symbol.for("cond");
export const OP_ELSE   = Symbol.for("else"); // part of cond but not a special form
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
    OP_LET,
    OP_IF, 
    OP_COND, 
    OP_AND, 
    OP_OR, 
    OP_BEGIN,
])

export const BUILTINS_OPS = new Set([
    // List Operations
    OP_LIST,
    OP_CONS,
    OP_CAR,
    OP_CDR,
    OP_LAST,
    OP_LENGTH,
    OP_EMPTY,
    OP_CONTAINS,
    OP_MAP,
    OP_APPLY,

    // Logic & Type Checking
    OP_NOT,
    OP_TYPE,
    OP_EQ,
    OP_EQ_PTR1,
    OP_EQ_PTR2,
    OP_EQ_DEEP1,
    OP_EQ_DEEP2,

    // Math & Comparisons
    OP_LT,
    OP_GT,
    OP_LTE,
    OP_GTE,
    OP_ADD,
    OP_SUB,
    OP_MUL,
    OP_DIV,
    OP_MODULO
]);

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
