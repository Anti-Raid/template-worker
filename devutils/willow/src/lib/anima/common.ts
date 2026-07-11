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

/** Properties that are exposed to the anima engine */
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

export class ErrorObject {
    constructor(public error: any) {}
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

export const SPECIAL_FORMS = new Set([
    OP_DEFINE, 
    OP_SET,
    OP_BEGIN,
    OP_LAMBDA, 
    OP_LET,
    OP_LETSTAR,
    OP_LETREC,
    OP_IF,
    OP_COND,
    OP_ELSE,
    OP_QUOTE,
    OP_AND,
    OP_OR
])

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

// Logic & Type Checking
export const OP_NOT      = Symbol.for("not");
export const OP_TYPE     = Symbol.for("type?");
export const OP_EQ       = Symbol.for("=");
export const OP_EQQ  = Symbol.for("eq?");
export const OP_EQV  = Symbol.for("eqv?");
export const OP_EQUAL = Symbol.for("equal?");

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
            if (token === '<#void>') return undefined;

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

        // Errors
        if (ast instanceof ErrorObject) {
            return `<error: ${ast.error?.message}>`
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

export const ensureCanBind = (param: any, seen: Set<symbol> | undefined, syntaxCtx: string, builtins: Map<symbol, number>) => {
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

    if (builtins.has(param)) {
        throw new Error(`${String(param)}: cannot shadow builtin procedure`)
    }
}

export type UnpackedLambdaArgs = { params: symbol[], remParams: symbol | null }
export const unpackLambdaExprArgs = (expr: any, builtins: Map<symbol, number>, ctx?: string): UnpackedLambdaArgs => {
    let params: symbol[] = []
    let remParams: symbol | null = null
    if (Array.isArray(expr[1])) {
        params = expr[1]
    } else if (expr[1] instanceof DottedPair) {
        // Bind params to items and remParam to remParams
        params = expr[1].items
        remParams = expr[1].rest
    } else if (typeof expr[1] === "symbol") {
        // Then all args must be bound to remparams
        remParams = expr[1]
    } else {
        throw new Error(`${ctx || "lambda"} arguments must be a symbol (to bind all as a list to said symbol) or a list`);
    }

    // Validate params and remParams here
    const seen = new Set<symbol>();
    for(let i = 0; i < params.length; i++) {
        ensureCanBind(params[i], seen, ctx || "lambda", builtins)
    }
    if (remParams) {
        ensureCanBind(remParams, seen, ctx || "lambda", builtins)
    }

    return { params, remParams }
}

export const flattenDynamicArgs = (actualArgs: any[], callerArgs: any[], start: number, nargs: number, ctx: string) => {
    const initialFill = actualArgs.length
    for (let i = 1+initialFill; i < nargs - 1; i++) {
        actualArgs.push(callerArgs[start + i]);
    }
    const finalArg = callerArgs[start + nargs - 1]
    if (Array.isArray(finalArg) || finalArg instanceof Cons) {
        actualArgs.push(...finalArg);
    } else if (finalArg === null) {
        // Empty list
    } else {
        throw new Error(`${ctx}: last argument must be a list but got ${String(finalArg)}`);
    }
    return actualArgs
}

export const wrapMulti = (exprs: any[]) => {
    if (exprs.length === 0) return []; 
    if (exprs.length === 1) return exprs[0];
    return [OP_BEGIN, ...exprs];
}

/**
 * Bytecode storage
 * Internal format:
 * <objtype><data>
 */
export class BS {
    #buffer: Uint32Array;
    #length: number = 0;
    #textEncoder = new TextEncoder();

    static readonly U32 = 0x01
    static readonly U32ARR = 0x02
    static readonly STR = 0x03
    static readonly SYMBOL = 0x04
    static readonly ARR = 0x05
    static readonly MAP = 0x06
    static readonly OBJ = 0x07
    static readonly NULL = 0x08
    static readonly BOOL = 0x09
    static readonly CLASS = 0x0A
    static readonly UNDEFINED = 0xFF

    constructor(initialCapacity: number = 1024) {
        this.#buffer = new Uint32Array(initialCapacity);
    }

    // Ensures we have enough space, doubling the buffer if necessary
    #ensureCapacity(needed: number) {
        if (this.#length + needed > this.#buffer.length) {
            const newSize = Math.max(this.#buffer.length * 2, this.#length + needed);
            const newBuffer = new Uint32Array(newSize);
            newBuffer.set(this.#buffer);
            this.#buffer = newBuffer;
        }
    }

    /** Write a single 32-bit word (e.g., an opcode or register index) 
     * 
     * Format: <U32><val>
    */
    writeU32(val: number): void {
        this.#ensureCapacity(2);
        this.#buffer[this.#length++] = BS.U32
        this.#buffer[this.#length++] = val
    }

    /** Write an array of 32-bit words
     * 
     * Format: <U32ARR><length><arr>
     */
    writeU32Arr(arr: Uint32Array): void {
        this.#ensureCapacity(2 + arr.length);
        this.#buffer[this.#length++] = BS.U32ARR
        this.#buffer[this.#length++] = arr.length
        this.#buffer.set(arr, this.#length);
        this.#length += arr.length;
    }

    /** Writes a string */
    writeString(str: string): void {
        return this.#writeString(str, BS.STR)
    }

    /** Writes a non-unique/interned/Symbol.for() symbol */
    writeSymbol(sym: symbol): void {
        const str = Symbol.keyFor(sym);
        if (str === undefined) throw new Error("Cannot write unique symbol")
        return this.#writeString(str, BS.SYMBOL)
    }

    /** * Writes a string (for symbols/constants). 
     * Format: <STR (op)><length><utf8 packed into 32-bit words>
     */
    #writeString(str: string, strop: number): void {
        const bytes = this.#textEncoder.encode(str);
        const wordsNeeded = Math.ceil(bytes.length / 4);
        this.#ensureCapacity(2 + wordsNeeded);
        this.#buffer[this.#length++] = strop
        this.#buffer[this.#length++] = bytes.length
        
        for (let i = 0; i < bytes.length; i += 4) {
            let word = 0;
            word |= (bytes[i] || 0);
            word |= (bytes[i + 1] || 0) << 8;
            word |= (bytes[i + 2] || 0) << 16;
            word |= (bytes[i + 3] || 0) << 24;
            this.#buffer[this.#length++] = word >>> 0;
        }
    }

    /** 
     * Writes a heterogeneous array containing any supported types 
     * 
     * Format: <ARR><LEN><VALS>
    */
    writeArray(arr: any[]): void {
        this.#ensureCapacity(2);
        this.#buffer[this.#length++] = BS.ARR;
        this.#buffer[this.#length++] = arr.length;
        for (const item of arr) {
            this.writeValue(item);
        }
    }

    /** Writes a Map (key-value pairs) */
    writeMap(map: Map<any, any>): void {
        this.#ensureCapacity(2);
        this.#buffer[this.#length++] = BS.MAP;
        this.#buffer[this.#length++] = map.size;
        for (const [key, val] of map.entries()) {
            this.writeValue(key);
            this.writeValue(val);
        }
    }

    /** Writes a plain JavaScript Object (Record<string, any>) */
    writeObject(obj: Record<string, any>): void {
        const entries = Object.entries(obj);
        this.#ensureCapacity(2);
        this.#buffer[this.#length++] = BS.OBJ;
        this.#buffer[this.#length++] = entries.length;
        for (const [key, val] of entries) {
            this.writeValue(key);
            this.writeValue(val);
        }
    }

    /** Writes a boolean value */
    writeBool(val: boolean): void {
        this.#ensureCapacity(2);
        this.#buffer[this.#length++] = BS.BOOL;
        this.#buffer[this.#length++] = val ? 1 : 0;
    }

    /** Writes a null value */
    writeNull(): void {
        this.#ensureCapacity(1);
        this.#buffer[this.#length++] = BS.NULL;
    }

    /** Writes a undefined value */
    writeUndefined(): void {
        this.#ensureCapacity(1);
        this.#buffer[this.#length++] = BS.UNDEFINED;
    }

    /** Writes a custom class implementing SerializableBytecode */
    writeSerializable(obj: SerializableBytecode): void {
        this.#ensureCapacity(1);
        this.#buffer[this.#length++] = BS.CLASS;
        this.writeString(obj.bsid);
        obj.dump(this);
    }


    /** Helper to dynamically write any supported type */
    writeValue(val: any): void {
        if (val === null) {
            this.writeNull();
        } else if (val === undefined) {
            this.writeUndefined()
        } else if (typeof val === 'number') {
            this.writeU32(val);
        } else if (typeof val === 'string') {
            this.writeString(val);
        } else if (typeof val === 'symbol') {
            this.writeSymbol(val);
        } else if (typeof val === 'boolean') {
            this.writeBool(val);
        } else if (val instanceof Uint32Array) {
            this.writeU32Arr(val);
        } else if (Array.isArray(val)) {
            this.writeArray(val);
        } else if (val instanceof Map) {
            this.writeMap(val);
        } else if (typeof val === 'object' && 'bsid' in val && 'dump' in val && typeof val.dump === 'function') {
            this.writeSerializable(val as SerializableBytecode);
        } else if (typeof val === 'object') {
            this.writeObject(val);
        } else {
            throw new Error(`Unsupported type for serialization: ${typeof val}`);
        }
    }

    /** Returns the final dumped bytecode array */
    finalize(): Uint32Array {
        return this.#buffer.slice(0, this.#length);
    }
}

export class BSReader {
    #buffer: Uint32Array;
    #cursor: number = 0;
    #textDecoder = new TextDecoder();
    #factories = new Map<string, (r: BSReader) => any>();

    constructor(buffer: Uint32Array) {
        this.#buffer = buffer;
    }

    get hasMore(): boolean {
        return this.#cursor < this.#buffer.length;
    }

    /**
     * Registers a factory function capable of deserializing a specific class type.
     * @param bsid The unique identifier matching the SerializableBytecode.bsid
     * @param factory A function that reads from the reader and returns the constructed class
     */
    registerFactory(bsid: string, factory: (r: BSReader) => any): void {
        this.#factories.set(bsid, factory);
    }

    /**
     * Peeks at the tag of the next value without advancing the cursor.
     */
    peekTag(): number {
        if (!this.hasMore) throw new Error("Unexpected end of bytecode");
        return this.#buffer[this.#cursor];
    }

    /**
     * Reads the next dynamically typed value based on its tag.
     */
    read(): number | Uint32Array | string | symbol | boolean | null | undefined | any[] | Map<any, any> | Record<string, any> {
        if (!this.hasMore) throw new Error("Unexpected end of bytecode");

        const tag = this.#buffer[this.#cursor++];

        switch (tag) {
            case BS.U32:
                return this.#buffer[this.#cursor++];
            
            case BS.U32ARR: {
                const len = this.#buffer[this.#cursor++];
                // We use slice to give a detached copy of the array chunk
                const arr = this.#buffer.slice(this.#cursor, this.#cursor + len);
                this.#cursor += len;
                return arr;
            }
            
            case BS.STR:
            case BS.SYMBOL: {
                const byteLen = this.#buffer[this.#cursor++];
                const wordsToRead = Math.ceil(byteLen / 4);
                const bytes = new Uint8Array(byteLen);
                
                let byteIndex = 0;
                for (let i = 0; i < wordsToRead; i++) {
                    const word = this.#buffer[this.#cursor++];
                    if (byteIndex < byteLen) bytes[byteIndex++] = word & 0xFF;
                    if (byteIndex < byteLen) bytes[byteIndex++] = (word >> 8) & 0xFF;
                    if (byteIndex < byteLen) bytes[byteIndex++] = (word >> 16) & 0xFF;
                    if (byteIndex < byteLen) bytes[byteIndex++] = (word >> 24) & 0xFF;
                }
                
                const str = this.#textDecoder.decode(bytes);
                return tag === BS.SYMBOL ? Symbol.for(str) : str;
            }

            case BS.ARR: {
                const len = this.#buffer[this.#cursor++];
                const arr = new Array(len);
                for (let i = 0; i < len; i++) {
                    arr[i] = this.read();
                }
                return arr;
            }

            case BS.MAP: {
                const len = this.#buffer[this.#cursor++];
                const map = new Map();
                for (let i = 0; i < len; i++) {
                    const key = this.read();
                    const val = this.read();
                    map.set(key, val);
                }
                return map;
            }

            case BS.OBJ: {
                const len = this.#buffer[this.#cursor++];
                const obj: Record<string, any> = Object.create(null);
                for (let i = 0; i < len; i++) {
                    const key = this.read();
                    const val = this.read();
                    obj[key as string] = val;
                }
                return obj;
            }

            case BS.NULL:
                return null;
            
            case BS.BOOL:
                return this.#buffer[this.#cursor++] === 1;

            case BS.CLASS: {
                // Read the class ID using the STR tag deserializer logic
                const bsid = this.readString();
                const factory = this.#factories.get(bsid);
                if (!factory) {
                    throw new Error(`no factory registered for SerializableBytecode class '${bsid}'`);
                }
                // The factory is expected to read its own internal state from the reader
                return factory(this);
            }

            case BS.UNDEFINED: {
                return undefined;
            }

            default:
                throw new Error(`Unknown data tag encountered: 0x${tag.toString(16)} at offset ${this.#cursor - 1}`);
        }
    }

    /** Helper to explicitly expect a U32 */
    readU32(): number {
        if (this.peekTag() !== BS.U32) throw new Error("Expected U32");
        const val = this.read();
        return val as number;
    }

    /** Helper to explicitly expect a Uint32Array */
    readU32Arr(): Uint32Array {
        if (this.peekTag() !== BS.U32ARR) throw new Error("Expected Uint32Array");
        return this.read() as Uint32Array;
    }

    /** Helper to explicitly expect a string */
    readString(): string {
        if (this.peekTag() !== BS.STR) throw new Error("Expected string");
        return this.read() as string;
    }

    /** Helper to explicitly expect a string */
    readSymbol(): symbol {
        if (this.peekTag() !== BS.SYMBOL) throw new Error("Expected string");
        return this.read() as symbol;
    }

    /** Helper to explicitly expect an Array */
    readArray(): any[] {
        if (this.peekTag() !== BS.ARR) throw new Error("Expected Array");
        return this.read() as any[];
    }

    /** Helper to explicitly expect a Map */
    readMap(): Map<any, any> {
        if (this.peekTag() !== BS.MAP) throw new Error("Expected Map");
        return this.read() as Map<any, any>;
    }

    /** Helper to explicitly expect an object */
    readObject(): Record<string, any> {
        if (this.peekTag() !== BS.OBJ) throw new Error("Expected Object");
        return this.read() as Record<string, any>;
    }

    /** Helper to explicitly expect an boolean */
    readBool(): boolean {
        if (this.peekTag() !== BS.BOOL) throw new Error("Expected bool");
        return this.read() as boolean;
    }

    /** Helper to explicitly expect an null */
    readNull(): null {
        if (this.peekTag() !== BS.NULL) throw new Error("Expected null");
        return this.read() as null;
    }

    /** Helper to explicitly expect an null */
    readUndefined(): undefined {
        if (this.peekTag() !== BS.UNDEFINED) throw new Error("Expected undefined");
        return this.read() as undefined;
    }

    /** Helper to explicitly expect a serializable */
    readSerializable<T extends SerializableBytecode>(expectedBsid?: string): T {
        if (this.peekTag() !== BS.CLASS) throw new Error("Expected class");
        const res = this.read() as T;
        if (expectedBsid !== undefined && res.bsid != expectedBsid) throw new Error(`Expected ${expectedBsid} but got ${res.bsid}`)
        return res
    }
}

export interface SerializableBytecode {
    bsid: string
    // Returns the underlying bytecode instructions as a Uint32array
    dump(w: BS): void;
}

/** A simple structure for registering constants */
export class ConstPool {
    #known: Map<unknown, number>;
    public constants: any[]
    constructor() {
        this.constants = []
        this.#known = new Map()

        // pre-reserve constants
        this.push(false)
        this.push(true)
        this.push(null)
        this.push(undefined)
    }

    // Register a object with the constant pool
    push(s: unknown) {
        // Try to deduplicate anything
        if (s === null || typeof s !== "object") {
            const idx = this.#known.get(s)
            if(idx !== undefined) {
                return idx
            } else {
                const idx = this.constants.push(s) - 1
                this.#known.set(s, idx)
                return idx
            }
        }

        // TODO: Deduplicate stuff later
        return this.constants.push(this.#freezeObj(s)) - 1
    }

    mutPush(s: unknown) {
        return this.constants.push(s) - 1
    }

    #freezeObj(obj: any) {
        if (typeof obj !== "object") return obj
        Object.keys(obj).forEach(prop => {
            if (typeof obj[prop] === 'object' && !Object.isFrozen(obj[prop])) {
                this.#freezeObj(obj[prop]);
            }
        });
        return Object.freeze(obj);
    }
}

export class Globals {
    private constructor(public data: Map<symbol, any>, public frozen: boolean = false, public outer: Globals | null) {}

    static newWith(fields: Record<symbol, any>, frozen: boolean = false) {
        const map = new Map()
        Object.getOwnPropertySymbols(fields).forEach((sym) => {
            map.set(sym, fields[sym])
        });
        return new Globals(map, frozen, null);
    }

    nestWith(fields: Record<symbol, any>, frozen: boolean = false) {
        const map = new Map()
        Object.getOwnPropertySymbols(fields).forEach((sym) => {
            map.set(sym, fields[sym])
        });
        return new Globals(map, frozen, this);
    }

    get(varname: symbol): any {
        if (this.data.has(varname)) {
            return this.data.get(varname)
        }
        if (this.outer) {
            return this.outer.get(varname)
        }
        throw new MissingVarError(`Variable '${String(varname)}' is not defined in the current scope.`)
    }

    assert(varname: symbol): void {
        if (this.data.has(varname)) {
            return
        }
        if (this.outer) {
            return this.outer.assert(varname)
        }
        throw new MissingVarError(`Variable '${String(varname)}' is not defined in the current scope.`)
    }

    set(varname: symbol, data: any) {
        if (this.frozen) throw new Error(`Variable '${String(varname)}' cannot be set in a frozen scope.`);
        this.data.set(varname, data)
    }
}
