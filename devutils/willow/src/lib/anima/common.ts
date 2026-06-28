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

        const iterA = a[Symbol.iterator]();
        const iterB = b[Symbol.iterator]();

        while (true) {
            const nextA = iterA.next();
            const nextB = iterB.next();

            if (nextA.done) {
                return isDeepEqual(nextA.value, nextB.value); 
            }

            // Compare the current elements
            if (!isDeepEqual(nextA.value, nextB.value)) {
                return false;
            }
        }
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

/** Properties that are exposed to the anima engine, can be retrieved with (ui-get propname) */
export class ExposedProps {
    #props: Record<string, any>;

    constructor(props: Record<string, any>) {
        this.#props = props
    }

    get(key: string): any {
        if (Object.hasOwn(this.#props, key)) {
            return this.#props[key]
        }
        return undefined
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
export const OP_CONTAINS = Symbol.for("contains?");
export const OP_MEMBER   = Symbol.for("member?");
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
export const OP_REMAINDER = Symbol.for("remainder")

// Misc
export const OP_UI_GET = Symbol.for("ui-get")

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
    OP_MEMBER,
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
    OP_MODULO,
    OP_REMAINDER,

    // Misc
    OP_UI_GET
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

// Represents a dotted pair, only supported in bytecode compiler
export class DottedPair {
    constructor(public items: any[], public rest: any) {}
}

export class ASP {    
    #str: string;
    #currPos: number;
    #supportsDottedPairs: boolean = false // only bytecode compiler supports these, AST interpreter does not
    constructor(str: string, supportsDottedPairs: boolean = false) {
        this.#str = str
        this.#currPos = 0
        this.#supportsDottedPairs = supportsDottedPairs
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

                    if (this.#supportsDottedPairs && tokens[current] === '.') {
                        current++; // consume .
                        if (tokens[current] === expectedClose) {
                            throw new Error(`Syntax error: trailing '.' is not allowed`);
                        }
                        // Parse rest and make sure its the final guy
                        const remParam = walk();
                        if (tokens[current] !== expectedClose) {
                            throw new Error(`Syntax error: multiple expressions after '.' is not allowed`);
                        }
                        current++;
                        return new DottedPair(lst, remParam)
                    }
                    lst.push(walk())
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

// Marker class that all procs should extend from
export class IProcedure {}

export class ASTStringifier {
    constructor() {}

    public stringify(ast: any): string {
        // Booleans+number
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
        if (ast === null) return "()";
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
        } else if (ast instanceof DottedPair) {
            const lst = new Array(ast.items.length)
            for(let i = 0; i < ast.items.length; i++) {
                lst[i] = this.stringify(ast.items[i]);
            }
            const rest = this.stringify(ast.rest);
            return `(${lst.join(" ")} . ${rest})`
        }

        // Procs
        if (ast instanceof IProcedure) {
            return `<procedure>`;
        }

        // Undefined
        if (ast === undefined) return `<#void>`

        throw new Error(`Cannot stringify unknown AST node: ${JSON.stringify(ast)}`);
    }
}

// Normalizes an expression
//
// In particular:
// - Converts all DottedPair's into Cons
export const normalizeExpr = (expr: any): any =>{
    // Try preserving array-ness as far as possible for performance purposes
    if (Array.isArray(expr)) {
        if (expr.length === 0) return null; 
        return expr.map(e => normalizeExpr(e))
    }

    if (expr instanceof DottedPair) {
        const items = expr.items.map(e => normalizeExpr(e));
        let tail = normalizeExpr(expr.rest);

        // Build the cons backwards as (1 2 . 3) => (cons 1 (cons 2 3))
        for (let i = items.length - 1; i >= 0; i--) {
            tail = Cons.pair(items[i], tail);
        }
        return tail;
    }

    return expr;
}

export const ensureCanBind = (param: any, seen: Set<symbol> | undefined, syntaxCtx: string) => {
    if(typeof param !== "symbol") {
        throw new Error(`${syntaxCtx} parameter must be a symbol, but received ${typeof param}: ${String(param)}`);
    }
    
    if (seen) {
        if (seen.has(param)) {
            throw new Error(`${syntaxCtx} parameter is a duplicate parameter name: ${String(param)}`);
        }
        seen.add(param)
    }

    if (SPECIAL_FORMS.has(param)) {
        throw new Error(`${String(param)}: bad syntax`)
    }
    if (BUILTINS_OPS.has(param)) {
        throw new Error(`${String(param)}: cannot shadow builtin procedure`)
    }
}
