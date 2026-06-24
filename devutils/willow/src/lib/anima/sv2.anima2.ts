import {
  OP_DEFINE,
  OP_BEGIN,
  OP_LAMBDA,
  OP_LET,
  OP_IF,
  OP_COND,
  OP_ELSE,
  OP_QUOTE,
  OP_AND,
  OP_OR,
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
  OP_NOT,
  OP_TYPE,
  OP_EQ,
  OP_EQ_PTR1,
  OP_EQ_PTR2,
  OP_EQ_DEEP1,
  OP_EQ_DEEP2,
  OP_LT,
  OP_GT,
  OP_LTE,
  OP_GTE,
  OP_ADD,
  OP_SUB,
  OP_MUL,
  OP_DIV,
  OP_MODULO,
  SPECIAL_FORMS,
  AnimaScope,
  BUILTINS_OPS,
  ASP,
  isTruthy,
  OP_SET,
  OP_LETREC,
  OP_LETSTAR,
  IProcedure,
  DottedPair,
  ASTStringifier,
  OP_REMAINDER
} from "./common";
import { Cons } from "./list";

export enum OpCode {
    // Push a constant from consts to the stack
    PUSH, 
    // Push specialization for `true`
    PUSH__TRUE,
    // Push specialization for `false`
    PUSH__FALSE,
    // Push specialization for `empty list`
    PUSH__EMPTYLIST,
    // Push specialization for `void`
    PUSH__VOID,
    // Push specialization for all numbers between 0 and 255
    PUSH__U8,
    // Negate whatevers at top of stack
    NEGATE,
    // Push a duplicate of the top of the stack to top of stack
    //
    // Needed so OP_AND/OP_OR can DUP, then JUMPIFFALSE, preserving top of stack after the jump
    DUP,
    // Pops out the top argument of the stack
    POP,
    // Get a variable from either the list of registered builtins or the current scope (GETVAR [varname-idx])
    GETVAR,
    // sets the top stack value on the stack on the current scope (SETVAR [varname-idx]), *always* pops stack top
    SETVAR,
    // defines the top stack value on the stack on the current scope (SETVAR [varname-idx]), *always* pops stack top
    DEFINEVAR,
    // Jump if stack top is true, *always* pops stack top
    JUMPIFTRUE,
    // Jump if stack top is false, *always* pops stack top
    JUMPIFFALSE,
    // Jump unconditionally
    JUMP,
    // Call a builtin or custom procedure (CALL nargs)
    CALL, 
    // Tail call a builtin or custom procedure (TAILCALL nargs). This reuses/overwrites the existing stack frame instead of creating a new one (like call does)
    TAILCALL,
    // Return from function with top value as return value. All other values are cleared from stack       
    RETURN,
    // Creates a Closure out of a ClosureTemplate (NEWCLOSURE idx) and pushes it to top of stack
    NEWCLOSURE,

    // Intrinsics

    // Given proc followed by nargs arguments pushed to stack followed by nargs in stack, perform a (apply proc args... rem-args-lst)
    INTRINSIC_APPLY, // non-tail-call (creates its own stack frame)
    INTRINSIC_TAIL_APPLY, // tail-call (overwrites existing frame)

    // Given n values on stack followed by nargs in stack, adds/subs/muls/divs all of them and pushes the result to stack
    INTRINSIC_ADD,
    INTRINSIC_SUB,
    INTRINSIC_MUL,
    INTRINSIC_DIV,
    INTRINSIC_EQ,

    // 2-arg, no nargs
    INTRINSIC_MODULO, 
    INTRINSIC_REMAINDER,

    // list, same as add/sub/mul/div in syntax
    INTRINSIC_LIST,
}

// TODO: Use LEB128 (thanks gemini for letting me know this exists!) to encode numbers
export class ByteCode {
    #knownSymbols: Map<symbol, number>;
    #knownNumbers: Map<number, number>;
    public constants: any[]
    public inst: number[]
    constructor() {
        this.constants = []
        this.inst = []
        this.#knownSymbols = new Map()
        this.#knownNumbers = new Map()
    }

    push(v: any) {
        if (typeof v === "symbol") {
            const symIdx = this.#knownSymbols.get(v)
            if(symIdx) {
                this.inst.push(OpCode.PUSH, symIdx)
            } else {
                const idx = this.constants.push(v) - 1
                this.#knownSymbols.set(v, idx)
                this.inst.push(OpCode.PUSH, idx)
            }
        } else if (typeof v === "number") {
            if (Number.isInteger(v) && v >= 0 && v <= 255) {
                // We can use u8 specialization here
                this.inst.push(OpCode.PUSH__U8, v);
                return
            } else if (Number.isInteger(v) && v >= -255 && v < 0) {
                // We can use u8 specialization here but we need to negate after pushing
                this.inst.push(OpCode.PUSH__U8, Math.abs(v));
                this.negate()
                return
            }
            // We need to use a normal push operation here
            const numIdx = this.#knownNumbers.get(v)
            if(numIdx) {
                this.inst.push(OpCode.PUSH, numIdx)
            } else {
                const idx = this.constants.push(v) - 1
                this.#knownNumbers.set(v, idx)
                this.inst.push(OpCode.PUSH, idx)
            }
        } else if (typeof v === "boolean") {
            this.inst.push(v ? OpCode.PUSH__TRUE : OpCode.PUSH__FALSE)
        } else if((Array.isArray(v) && v.length === 0) || v === null) {
            this.inst.push(OpCode.PUSH__EMPTYLIST)
        } else if (v === undefined) {
            this.inst.push(OpCode.PUSH__VOID)
        } else {
            // TODO: Deduplicate stuff later once this actually works
            const idx = this.constants.push(this.#freezeObj(v)) - 1
            this.inst.push(OpCode.PUSH, idx)
        }
    }
    negate() { this.inst.push(OpCode.NEGATE) }
    dup() { this.inst.push(OpCode.DUP) }
    pop() { this.inst.push(OpCode.POP) }
    getVar(varname: symbol) {
        const symIdx = this.#knownSymbols.get(varname)
        if(symIdx) {
            this.inst.push(OpCode.GETVAR, symIdx)
        } else {
            const idx = this.constants.push(varname) - 1
            this.#knownSymbols.set(varname, idx)
            this.inst.push(OpCode.GETVAR, idx)
        }
    }
    defineVar(varname: symbol) {
        const symIdx = this.#knownSymbols.get(varname)
        if(symIdx) {
            this.inst.push(OpCode.DEFINEVAR, symIdx)
        } else {
            const idx = this.constants.push(varname) - 1
            this.#knownSymbols.set(varname, idx)
            this.inst.push(OpCode.DEFINEVAR, idx)
        }
    }
    setVar(varname: symbol) {
        const symIdx = this.#knownSymbols.get(varname)
        if(symIdx) {
            this.inst.push(OpCode.SETVAR, symIdx)
        } else {
            const idx = this.constants.push(varname) - 1
            this.#knownSymbols.set(varname, idx)
            this.inst.push(OpCode.SETVAR, idx)
        }
    }
    jumpIfTrue() {
        this.inst.push(OpCode.JUMPIFTRUE)
        return this.inst.push(-1) - 1 // to be replaced by compiler
    }
    jumpIfFalse() {
        this.inst.push(OpCode.JUMPIFFALSE)
        return this.inst.push(-1) - 1 // to be replaced by compiler
    }
    jump() {
        this.inst.push(OpCode.JUMP)
        return this.inst.push(-1) - 1 // to be replaced by compiler
    }
    setJumpToCurrent(jumpIdx: number) {
        if (this.inst[jumpIdx] != -1) throw new Error("internal error: setJumpToCurrent not called on unfilled jump idx")
        this.inst[jumpIdx] = this.inst.length; 
    }

    call(args: number) { this.inst.push(OpCode.CALL, args) }
    tailcall(args: number) { this.inst.push(OpCode.TAILCALL, args) }
    return() { this.inst.push(OpCode.RETURN) }
    newclosure(tmplInfo: ClosureTemplate) {
        const idx = this.constants.push(tmplInfo) - 1
        this.inst.push(OpCode.NEWCLOSURE, idx)
    } 

    intrinsicApply() {
        this.inst.push(OpCode.INTRINSIC_APPLY)
    }

    intrinsicTailApply() {
        this.inst.push(OpCode.INTRINSIC_TAIL_APPLY)
    }

    intrinsicAdd() {
        this.inst.push(OpCode.INTRINSIC_ADD)
    }

    intrinsicSub() {
        this.inst.push(OpCode.INTRINSIC_SUB)
    }

    intrinsicMul() {
        this.inst.push(OpCode.INTRINSIC_MUL)
    }

    intrinsicDiv() {
        this.inst.push(OpCode.INTRINSIC_DIV)
    }

    intrinsicEq() {
        this.inst.push(OpCode.INTRINSIC_EQ)
    }

    intrinsicModulo() {
        this.inst.push(OpCode.INTRINSIC_MODULO)
    }

    intrinsicRemainder() {
        this.inst.push(OpCode.INTRINSIC_REMAINDER)
    }

    intrinsicList() {
        this.inst.push(OpCode.INTRINSIC_LIST)
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

    #constToString(s: any): string {
        if(typeof s === "symbol") {
            return `'${s.toString()}`
        } else if (typeof s === "string") {
            return `"${s.toString()}"`
        } else if (typeof s === "number") {
            return `${s}`
        } else if (typeof s === "boolean") {
            return `<${s}>`
        } else if (typeof s === "undefined") {
            return `#<void>`
        } else if (Array.isArray(s)) {
            const r = []
            for(const elem of s) {
                r.push(this.#constToString(elem))
            }
            return `[${r.join(', ')}]`
        } else {
            return `<unknown:${s}>`
        }
    }

    toString(): string {
        let ops: string[] = [];
        let idx = 0;
        
        while (idx < this.inst.length) {
            const lineNum = idx.toString().padStart(4, '0');
            const opcode = this.inst[idx];
            let line = `${lineNum}: `;

            switch (opcode) {
                // 1 arg
                case OpCode.PUSH:
                    line += `PUSH ${this.#constToString(this.constants[this.inst[idx + 1]])}`;
                    idx += 2;
                    break;
                case OpCode.PUSH__U8:
                    line += `PUSH__U8 ${this.inst[idx + 1]}`;
                    idx += 2;
                    break;
                case OpCode.GETVAR:
                    line += `GETVAR ${this.#constToString(this.constants[this.inst[idx + 1]])}`;
                    idx += 2;
                    break;
                case OpCode.DEFINEVAR:
                    line += `DEFINEVAR ${this.#constToString(this.constants[this.inst[idx + 1]])}`;
                    idx += 2;
                    break;
                case OpCode.SETVAR:
                    line += `SETVAR ${this.#constToString(this.constants[this.inst[idx + 1]])}`;
                    idx += 2;
                    break;
                case OpCode.JUMPIFTRUE:
                    line += `JUMPIFTRUE -> ${this.inst[idx + 1]}`;
                    idx += 2;
                    break;
                case OpCode.JUMPIFFALSE:
                    line += `JUMPIFFALSE -> ${this.inst[idx + 1]}`;
                    idx += 2;
                    break;
                case OpCode.JUMP:
                    line += `JUMP -> ${this.inst[idx + 1]}`;
                    idx += 2;
                    break;
                case OpCode.CALL:
                    line += `CALL (args: ${this.inst[idx + 1]})`;
                    idx += 2;
                    break;
                case OpCode.TAILCALL:
                    line += `TAILCALL (args: ${this.inst[idx + 1]})`;
                    idx += 2;
                    break;
                case OpCode.NEWCLOSURE:
                    const tmpl = this.constants[this.inst[idx + 1]] as ClosureTemplate;
                    const params = tmpl.params.map(p => p.toString()).join(" ");
                    line += `NEWCLOSURE <fn(${params})>`;
                    idx += 2;
                    break;

                // no arg
                case OpCode.PUSH__TRUE:
                    line += `PUSH__TRUE`;
                    idx += 1;
                    break;
                case OpCode.PUSH__FALSE:
                    line += `PUSH__FALSE`;
                    idx += 1;
                    break;
                case OpCode.PUSH__EMPTYLIST:
                    line += `PUSH__EMPTYLIST`;
                    idx += 1;
                    break;
                case OpCode.PUSH__VOID:
                    line += `PUSH__VOID`;
                    idx += 1;
                    break;
                case OpCode.NEGATE:
                    line += `NEGATE`
                    idx += 1;
                    break;
                case OpCode.DUP:
                    line += `DUP`;
                    idx += 1;
                    break;
                case OpCode.POP:
                    line += `POP`;
                    idx += 1;
                    break;
                case OpCode.RETURN:
                    line += `RETURN`;
                    idx += 1;
                    break;

                // vm intrinsics
                case OpCode.INTRINSIC_APPLY:
                    line += `INTRINSIC_APPLY`
                    idx += 1;
                    break;
                case OpCode.INTRINSIC_TAIL_APPLY:
                    line += `INTRINSIC_TAIL_APPLY`
                    idx += 1;
                    break;
                case OpCode.INTRINSIC_ADD:
                    line += `INTRINSIC_ADD`;
                    idx += 1
                    break;
                case OpCode.INTRINSIC_SUB:
                    line += `INTRINSIC_SUB`;
                    idx += 1
                    break;
                case OpCode.INTRINSIC_MUL:
                    line += `INTRINSIC_MUL`;
                    idx += 1
                    break;
                case OpCode.INTRINSIC_DIV:
                    line += `INTRINSIC_DIV`;
                    idx += 1
                    break;
                case OpCode.INTRINSIC_MODULO:
                    line += `INTRINSIC_MODULO`
                    idx += 1
                    break
                case OpCode.INTRINSIC_REMAINDER:
                    line += `INTRINSIC_REMAINDER`
                    idx += 1
                    break
                case OpCode.INTRINSIC_LIST:
                    line += `INTRINSIC_LIST`
                    idx += 1
                    break
                case OpCode.INTRINSIC_EQ:
                    line += `INTRINSIC_EQ`
                    idx += 1
                    break
                default:
                    line += `UNKNOWN_OPCODE (${opcode})`;
                    idx += 1;
                    break;
            }
            ops.push(line);
        }
        return ops.join("\n");
    }
}

interface CmpOpts {
    leaveOnStack: boolean // whether to leave created values on the stack or not
    isTail: boolean // whether this is a tail-call or not (for tco)
    bc: ByteCode
    tryOpt: boolean
    disableDefine: boolean
    disableLambda: boolean
    disableSet: boolean
}

export class AnimaCompiler {
    s = new ASTStringifier()
    t = new AnimaTransformer()
    o = new ASTOptimizer()

    compileStr(s: string, disableLambda: boolean = false, disableSet: boolean = false, tryOpt = true) {
        return this.compileExpr(new ASP(s, true).parse(), disableLambda, disableSet, tryOpt)
    }

    compileExpr(expr: any, disableDefine: boolean = false, disableLambda: boolean = false, disableSet: boolean = false, tryOpt = true) {
        // First transform the expr so all conds are resolved
        let trExpr = this.t.transform(expr)
        if (tryOpt) trExpr = this.o.optimize(trExpr)

        const bc = new ByteCode();
        this.#compile(trExpr, {leaveOnStack: true, isTail: true, bc, tryOpt, disableDefine, disableLambda, disableSet })
        return bc
    }
    #compile(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        // Raw values
        if (typeof expr === 'symbol') {
            opts.bc.getVar(expr)
            if (!opts.leaveOnStack) opts.bc.pop()
            return
        } else if (typeof expr === "string") {
            if (!opts.leaveOnStack && opts.tryOpt) return 
            opts.bc.push(expr)
            if (!opts.leaveOnStack) opts.bc.pop()
            return
        } else if (expr instanceof DottedPair) {
            throw new Error(`bad syntax: illegal use of dotted pair in execution context (consider quoting e.g. ${`'${this.s.stringify(expr)}`})`);
        } else if (!Array.isArray(expr)) {
            if (!opts.leaveOnStack && opts.tryOpt) return 
            opts.bc.push(expr)
            if (!opts.leaveOnStack) opts.bc.pop()
            return
        }

        if (expr.length === 0) {
            // An empty array evaluates to null
            if (!opts.leaveOnStack && opts.tryOpt) return 
            opts.bc.push([]) // specializes internally to PUSH__EMPTYLIST
            if (!opts.leaveOnStack) opts.bc.pop()
            return
        }

        const operator = expr[0];

        if (typeof operator === "symbol") {
            switch (operator) {
                case OP_BEGIN:
                    this.#compileBegin(expr, opts, syntaxCtx)
                    return
                case OP_IF:
                    this.#compileIfCall(expr, opts, syntaxCtx)
                    return
                case OP_COND:
                    throw new Error("internal error: cond should be transformed by AnimaTransform prior to reaching here")
                case OP_QUOTE:
                    this.#compileQuote(expr, opts, syntaxCtx)
                    return
                case OP_DEFINE:
                    this.#compileDefine(expr, opts, syntaxCtx)
                    return
                case OP_SET:
                    this.#compileSet(expr, opts, syntaxCtx)
                    return
                case OP_LAMBDA:
                    this.#compileLambda(expr, opts, syntaxCtx)
                    return
                case OP_LET:
                    throw new Error("internal error: let should be transformed by AnimaTransform prior to reaching here")
                case OP_LETSTAR:
                    throw new Error("internal error: let* should be transformed by AnimaTransform prior to reaching here")
                case OP_LETREC:
                    throw new Error("internal error: letrec should be transformed by AnimaTransform prior to reaching here")
                case OP_AND:
                    this.#compileAnd(expr, opts, syntaxCtx)
                    return
                case OP_OR:
                    this.#compileOr(expr, opts, syntaxCtx)
                    return
                // Intrinsic optimizations
                case OP_APPLY:
                    this.#optApply(expr, opts, syntaxCtx)
                    return
                case OP_ADD:
                case OP_SUB:
                case OP_MUL:
                case OP_DIV:
                case OP_EQ:
                    this.#optIntrinsicNormal(expr, opts, syntaxCtx)
                    return
                case OP_MODULO:
                case OP_REMAINDER:
                    this.#optIntrinsicTwoArgs(expr, opts, syntaxCtx)
                    return
                case OP_LIST:
                    this.#optList(expr, opts, syntaxCtx)
                    return
            }
        }

        this.#compileNormalCall(expr, opts)
    }

    #compileBegin(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // We need to push a void if we see an empty begin block
        if (expr.length === 1) {
            if (opts.leaveOnStack) opts.bc.push(undefined);
            return;
        }

        for (let i = 1; i < expr.length; i++) {
            // the child is a tail call only if we are a tail call and its the last child
            const isLastChild = (i === expr.length - 1);
            const childIsTail = isLastChild && opts.isTail; 
            this.#compile(expr[i], { ...opts, leaveOnStack: isLastChild, isTail: childIsTail });
        }
    }

    // a normal call
    #compileNormalCall(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // We need to compile the first arg first and leave it on the stack
        //
        // This will place a e.g. (PUSH) <symbol>
        this.#compile(expr[0], { ...opts, leaveOnStack: true, isTail: false })

        // Push all arguments
        const nargs = expr.length-1 // [func a b c] -> 5 - 2 = 3 args
        for(let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false })
        }

        if (opts.isTail) {
            opts.bc.tailcall(nargs)
        } else {
            opts.bc.call(nargs)
            // Popping only matters in non-tail-calls
            if (!opts.leaveOnStack) opts.bc.pop()
        }
    }

    // compiles both if calls as well as code that is converted into if calls
    #compileIfCall(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if(expr.length != 4) {
            throw new Error(`if condition must be in format ["if", condition, true_expr, false_expr] but only have ${expr.length-1} arguments`)
        }

        // We need to compile the first arg first and leave it on the stack
        // to act as a condition
        //
        // This will place a e.g. (PUSH) <symbol>
        const blen = opts.bc.inst.length
        this.#compile(expr[1], { ...opts, leaveOnStack: true, isTail: false })
        if (opts.bc.inst.length === blen) {
            throw new Error("internal error: compileIfCall -> compiling expr[0] yielded no pushed values on stack for condition")
        }
        // we place the bytecode as <jumpiffalse [false code]><true code><jump [|]><false code>|
        const jifIdx = opts.bc.jumpIfFalse();
        // Place true code
        this.#compile(expr[2], opts)
        // Place jump to end
        const tjIdx = opts.bc.jump()
        // Place false code as well as jump to start of false code
        opts.bc.setJumpToCurrent(jifIdx)
        this.#compile(expr[3], opts)
        // Fix jump to end to now jump to after false code
        opts.bc.setJumpToCurrent(tjIdx)
    }

    #wrapMulti = (exprs: any[]) => {
        if (exprs.length === 0) return []; 
        if (exprs.length === 1) return exprs[0];
        return [OP_BEGIN, ...exprs];
    }

    // Normalizes an expression
    //
    // In particular:
    // - Converts all DottedPair's into Cons
    #normalizeExpr(expr: any): any {
        // Try preserving array-ness as far as possible for performance purposes
        if (Array.isArray(expr)) {
            if (expr.length === 0) return null; 
            return expr.map(e => this.#normalizeExpr(e))
        }

        if (expr instanceof DottedPair) {
            const items = expr.items.map(e => this.#normalizeExpr(e));
            let tail = this.#normalizeExpr(expr.rest);

            // Build the cons backwards as (1 2 . 3) => (cons 1 (cons 2 3))
            for (let i = items.length - 1; i >= 0; i--) {
                tail = Cons.pair(items[i], tail);
            }
            return tail;
        }

        return expr;
    }

    #compileQuote(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if(expr.length != 2) {
            throw new Error(`quote must be in format ["quote", expr] but have ${expr.length-1} arguments`)
        }
        if(!opts.leaveOnStack) return
        opts.bc.push(this.#normalizeExpr(expr[1]))
    }

    #compileDefine(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if (opts.disableDefine) {
            throw new Error("define expressions are disabled in this context");
        }

        if(expr.length < 3) {
            throw new Error(`define must be in format ["define" varname arg] or [define (func_name arg1 arg2... argN) body_expr...] but have ${expr.length-1} arguments`)
        }

        if(typeof expr[1] !== "symbol") throw new Error("internal error: complex defines should be transformed by AnimaTransform prior to reaching here")

        // By now, everything here should be a normal define
        this.#ensureCanBind(expr[1], undefined, syntaxCtx || "define")

        // We need to compile the second arg first and leave it on the stack
        //
        // This will place a e.g. (PUSH) <symbol>
        this.#compile(expr[2], { ...opts, leaveOnStack: true, isTail: false });
        opts.bc.defineVar(expr[1])
        if (opts.leaveOnStack) {
            opts.bc.push(undefined);
        }
    }

    #compileSet(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if (opts.disableSet) {
            throw new Error("set! expressions are disabled in this context");
        }

        if(expr.length != 3) {
            throw new Error(`set! must be in format ["set!" varname arg] but have ${expr.length-1} arguments`)
        }

        // Normal define
        if(typeof expr[1] !== "symbol") {
            throw new Error(`${String(expr[1])}: expr[1] not symbol or list syntax`)
        }

        this.#ensureCanBind(expr[1], undefined, syntaxCtx || "set!")

        // We need to compile the second arg first and leave it on the stack
        //
        // This will place a e.g. (PUSH) <symbol>
        this.#compile(expr[2], { ...opts, leaveOnStack: true, isTail: false });
        opts.bc.setVar(expr[1])
        if (opts.leaveOnStack) {
            opts.bc.push(undefined);
        }
    }

    #ensureCanBind(param: any, seen: Set<symbol> | undefined, syntaxCtx: string) {
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
        if (param in BUILTINS_OPS) {
            throw new Error(`${String(param)}: cannot shadow builtin procedure`)
        }
    }

    #compileLambda(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if(opts.disableLambda) {
            throw new Error(`${syntaxCtx || "lambda"} expressions are disabled in this context [when compiling a lambda]`)
        }

        if(expr.length < 3) {
            throw new Error(`lambda must be in format ["lambda", [bind-args...], body...] but only have ${expr.length-1} arguments`)
        }

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
            throw new Error(`${syntaxCtx || "lambda"} arguments must be a symbol (to bind all as a list to said symbol) or a list`);
        }

        // Validate params and remParams here
        const seen = new Set<symbol>();
        for(let i = 0; i < params.length; i++) {
            this.#ensureCanBind(params[i], seen, syntaxCtx || "lambda")
        }
        if (remParams) this.#ensureCanBind(remParams, seen, syntaxCtx || "lambda")

        // Once we've verified the syntax, we can then drop the entire lambda if its not actually needed on the stack
        //
        // This lets us keep the syntax checking without the work
        if (!opts.leaveOnStack) return

        // Compile lambda body
        const lambdaBc = new ByteCode()
        this.#compile(this.#wrapMulti(expr.slice(2)), {...opts, leaveOnStack: true, isTail: true, bc: lambdaBc})
        lambdaBc.return()
        const template = new ClosureTemplate(params, remParams, lambdaBc);
        opts.bc.newclosure(template)
    }

    #compileAnd(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // if (argCount === 0) return true; 
        if (expr.length === 1) {
            if (opts.leaveOnStack) opts.bc.push(true);
            return;
        }

        // For every arg (excluding the tail cond), keep a list of jumps
        // which we will patch later to go to the end of the and block
        const jumpIndexes: number[] = [];

        for (let i = 1; i < expr.length - 1; i++) {
            // We need to compile the argument leave it on the stack
            // to act as a condition
            //
            // This will place a e.g. (PUSH) <symbol>
            this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false });

            if (opts.leaveOnStack) {
                // If parent wants a return value to be left on stack,
                // we cannot just jumpIfFalse as the jump will pop the return value
                //
                // Instead, DUP the top of stack so jumpIfFalse then pops the duplicate
                // with the original (falsey) value left safely on top of stack 
                opts.bc.dup();
                jumpIndexes.push(opts.bc.jumpIfFalse()); // and short-circuits if false
                // If we never jumped, pop the original return value and move on to the next cond                
                opts.bc.pop(); 
            } else {
                // If the parent doesn't care about the return value (e.g. inside a 'begin' block),
                // we can just jumpIfFalse which will pop from top of stack leaving no ret values on
                // top of stack
                jumpIndexes.push(opts.bc.jumpIfFalse());
            }
        }

        // tail expr is the last cond so it gets directly evaluated (if all the and conds reach)
        this.#compile(expr[expr.length - 1], opts);

        for (let i = 0; i < jumpIndexes.length; i++) {
            opts.bc.setJumpToCurrent(jumpIndexes[i]);
        }
    }

    #compileOr(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        // if (argCount === 0) return false;
        if (expr.length === 1) {
            if (opts.leaveOnStack) opts.bc.push(false);
            return;
        }

        // For every arg (excluding the tail cond), keep a list of jumps
        // which we will patch later to go to the end of the and block
        const jumpIndexes: number[] = [];

        for (let i = 1; i < expr.length - 1; i++) {
            // We need to compile the argument leave it on the stack
            // to act as a condition
            //
            // This will place a e.g. (PUSH) <symbol>
            this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false });

            if (opts.leaveOnStack) {
                // If parent wants a return value to be left on stack,
                // we cannot just jumpIfTrue as the jump will pop the return value
                //
                // Instead, DUP the top of stack so jumpIfTrue then pops the duplicate
                // with the original (falsey) value left safely on top of stack 
                opts.bc.dup();
                jumpIndexes.push(opts.bc.jumpIfTrue()); // or short-circuits if true
                // If we never jumped, pop the original return value and move on to the next cond                
                opts.bc.pop(); 
            } else {
                // If the parent doesn't care about the return value (e.g. inside a 'begin' block),
                // we can just jumpIfTrue which will pop from top of stack leaving no ret values on
                // top of stack
                jumpIndexes.push(opts.bc.jumpIfTrue());
            }
        }

        // tail expr is the last cond so it gets directly evaluated (if all the and conds reach)
        this.#compile(expr[expr.length - 1], opts);

        for (let i = 0; i < jumpIndexes.length; i++) {
            opts.bc.setJumpToCurrent(jumpIndexes[i]);
        }
    }

    /** Optimizes a direct (apply proc elems... rem-arg-lst) to inline INTRINSIC_APPLY */ 
    #optApply(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if (expr.length < 3) {
            throw new Error("apply requires at least a procedure and a list");
        }

        // Push proc
        this.#compile(expr[1], { ...opts, leaveOnStack: true, isTail: false });
        // Push args
        const nargs = expr.length - 2; // expr - Symbol(apply) - proc
        for(let i = 2; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false });
        }
        // Due to the vararg builtin function, INTRINSIC_APPLY also expects nargs at top of the stack
        opts.bc.push(nargs);
        // Now we're ready to do a INTRINSIC_APPLY 
        if (opts.isTail) {
            opts.bc.intrinsicTailApply()
        } else {
            opts.bc.intrinsicApply()

            // Popping only matters in non-tail apply's
            if (!opts.leaveOnStack) {
                opts.bc.pop();
            }
        }
    }

    /** Optimizes intrinsic ops to a INTRINSIC_ADD/SUB/MUL/DIV */ 
    #optIntrinsicNormal(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        const op = expr[0]

        // Push args
        const nargs = expr.length - 1; // expr - Symbol(op)
        for(let i = 1; i < expr.length; i++) {
            this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false });
        }
        // Due to the vararg builtin function, normal intrinsics also expects nargs at top of the stack
        opts.bc.push(nargs);
        // Now we're ready to do the intrinsic
        if (op === OP_ADD) opts.bc.intrinsicAdd()
        else if (op === OP_SUB) opts.bc.intrinsicSub()
        else if (op === OP_MUL) opts.bc.intrinsicMul()
        else if (op === OP_DIV) opts.bc.intrinsicDiv()
        else if (op === OP_EQ) opts.bc.intrinsicEq()
        else throw new Error(`internal error: no intrinsic for op ${op}`)
        if (!opts.leaveOnStack) {
            opts.bc.pop();
        }
    }

    #optIntrinsicTwoArgs(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        const op = expr[0]

        if (expr.length !== 3) {
            throw new Error(`${op} requires exactly 2 arguments, got ${expr.length - 1}`);
        }

        this.#compile(expr[1], { ...opts, leaveOnStack: true, isTail: false });
        this.#compile(expr[2], { ...opts, leaveOnStack: true, isTail: false });
        
        if (op === OP_MODULO) opts.bc.intrinsicModulo();
        if (op === OP_REMAINDER) opts.bc.intrinsicRemainder();
        
        if (!opts.leaveOnStack) {
            opts.bc.pop();
        }
    }

    /** Optimizes list to an INTRINSIC_LIST */ 
    #optList(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        const op = expr[0]

        // Push args
        const nargs = expr.length - 1; // expr - Symbol(op)
        if (nargs === 0) {
            // We can optimize this down to an empty list
            opts.bc.push([])
        } else {
            for(let i = 1; i < expr.length; i++) {
                this.#compile(expr[i], { ...opts, leaveOnStack: true, isTail: false });
            }
            // Due to the vararg builtin function, normal intrinsics also expects nargs at top of the stack
            opts.bc.push(nargs);
            opts.bc.intrinsicList()
        }
        if (!opts.leaveOnStack) {
            opts.bc.pop();
        }
    }
}

/** A template for a closure that can then be bound to a scope */
export class ClosureTemplate {
    params: symbol[]; // base (individual param binds)
    remParams: symbol | null; // where the remaining params should be bound too (if any). This implicitly makes a closure variadic as well
    code: ByteCode

    constructor(params: symbol[], remParams: symbol | null, code: ByteCode) {
        this.params = params
        this.remParams = remParams
        this.code = code
    }
}

/** An actual anima closure bound to a scope */
export class Closure extends IProcedure {
    constructor(public tmpl: ClosureTemplate, public scope: AnimaScope) {
        super()
    }
}

/** 
 * A builtin function. Builtin functions do not have access to their own lexical scope (at least not yet) 
 * 
 * 
 * Unlike normal functions which pop from stack and bind to a new AnimaScope, builtin funcs keep values on
 * stack and just do bytecode replacement
*/
export class BuiltinFunction extends IProcedure {
    // number of args needed on stack top, -1 means variadic
    // if we are in variadic mode, the top of stack will contain 
    // the number of arguments pushed on the stack
    nargs: number 
    bc: ByteCode
    needsScope: boolean // do we need scope or not (if false, this will use the global execution scope as the scope in bytecode)

    constructor(nargs: number, needsScope: boolean, initializer: (bc: ByteCode) => void) {
        super()
        this.nargs = nargs
        this.needsScope = needsScope
        this.bc = new ByteCode()
        initializer(this.bc)
    }
}

const BUILTINS_APPLY = new BuiltinFunction(-1, false, (bc) => {
    // note to self: its vararg, so nargs is at top of stack. As this is a stub that exists to trigger
    // the intrinsic, the apply is also in tail position, so emit a tail apply instead of apply
    bc.intrinsicTailApply()
});

const BUILTINS_ADD = new BuiltinFunction(-1, false, (bc) => {
    // note to self: its vararg, so nargs is at top of stack
    bc.intrinsicAdd()
});

const BUILTINS_SUB = new BuiltinFunction(-1, false, (bc) => {
    // note to self: its vararg, so nargs is at top of stack
    bc.intrinsicSub()
});

const BUILTINS_MUL = new BuiltinFunction(-1, false, (bc) => {
    // note to self: its vararg, so nargs is at top of stack
    bc.intrinsicMul()
});

const BUILTINS_DIV = new BuiltinFunction(-1, false, (bc) => {
    // note to self: its vararg, so nargs is at top of stack
    bc.intrinsicDiv()
});

const BUILTINS_MODULO = new BuiltinFunction(2, false, (bc) => {
    bc.intrinsicModulo()
});

const BUILTINS_REMAINDER = new BuiltinFunction(2, false, (bc) => {
    bc.intrinsicRemainder()
});

const BUILTINS_LIST = new BuiltinFunction(-1, false, (bc) => {
    bc.intrinsicList()
});

const BUILTINS_EQ = new BuiltinFunction(-1, false, (bc) => {
    bc.intrinsicEq()
})

export class NativeFunction extends IProcedure {
    constructor(
        public name: string,
        public nargs: number, // -1 for variadic
        public cb: (...args: any[]) => any
    ) {
        super()
    }
}

/** Registry of all builtin builtin procedures */
export const BUILTIN_PROCS: Record<symbol, BuiltinFunction | NativeFunction | Closure> = {
    [OP_APPLY]: BUILTINS_APPLY,
    [OP_ADD]: BUILTINS_ADD,
    [OP_SUB]: BUILTINS_SUB,
    [OP_MUL]: BUILTINS_MUL,
    [OP_DIV]: BUILTINS_DIV,
    [OP_MODULO]: BUILTINS_MODULO,
    [OP_REMAINDER]: BUILTINS_REMAINDER,
    [OP_LIST]: BUILTINS_LIST,
    [OP_EQ]: BUILTINS_EQ,
    // @ts-ignore
    __proto__: null
}

class CallFrame {
    constructor(
        public code: ByteCode,
        public scope: AnimaScope,
        public ip: number
    ) {}

    readNext() {
        if (this.ip >= this.code.inst.length) {
            throw new Error(`internal error: unexpected end of bytecode, instruction pointer out of bounds (${this.ip} >= ${this.code.inst.length}).`);
        }
        return this.code.inst[this.ip++]
    }

    getConst(idx: number) {
        return this.code.constants[idx]
    }
}

export class AnimaVM {
    constructor(public steps: number = 0, public maxSteps: number = 0) {}

    public evaluate(code: ByteCode, rawData: Record<string, any>): any {
        const globalScope = new AnimaScope(rawData, null)
        const executionScope = globalScope.nest(); // Any "define" calls now write to this temporary scope
        return this.#evalinner(code, executionScope);
    }

    public evaluateExpr(expr: any, disableDefine: boolean, disableLambda: boolean, rawData: Record<string, any>): any {
        const bc = (new AnimaCompiler()).compileExpr(expr, disableDefine, disableLambda)
        return this.evaluate(bc, rawData);
    }

    public evaluateStr(expr: string, rawData: Record<string, any>, disableDefine: boolean = false, disableLambda: boolean = false): any {
        const bc = (new AnimaCompiler()).compileStr(expr, disableDefine, disableLambda)
        return this.evaluate(bc, rawData);
    }

    #evalinner(code: ByteCode, execScope: AnimaScope): any {
        // Initial frame and stack
        let frames: CallFrame[] = [new CallFrame(code, execScope, 0)];
        let stack: any[] = [];

        while (frames.length > 0) {
            this.steps++;
            if (this.maxSteps && this.steps > this.maxSteps) {
                throw new Error(`Execution Limits Exceeded: Script ran for more than ${this.maxSteps} instructions.`);
            }

            const frame = frames[frames.length - 1];

            // If we've reached the end of the call frame, pop
            if (frame.ip >= frame.code.inst.length) {
                frames.pop();
                continue;
            }

            const opcode = frame.readNext()
            switch (opcode) {
                // Push
                case OpCode.PUSH: {
                    const constIdx = frame.readNext()
                    stack.push(frame.getConst(constIdx));
                    break;
                }
                // Push specializations
                case OpCode.PUSH__TRUE: {
                    stack.push(true)
                    break
                }
                case OpCode.PUSH__FALSE: {
                    stack.push(false)
                    break
                }
                case OpCode.PUSH__EMPTYLIST: {
                    stack.push(null) // empty list is null
                    break
                }
                case OpCode.PUSH__VOID: {
                    stack.push(undefined)
                    break
                }
                case OpCode.PUSH__U8: {
                    stack.push(frame.readNext())
                    break
                }
                case OpCode.NEGATE: {
                    const v = stack[stack.length-1]
                    if (typeof v !== "number") throw new Error("cannot negate non-number")
                    stack[stack.length-1] = -1*v
                    break
                }
                case OpCode.DUP: {
                    stack.push(stack[stack.length-1])
                    break
                }
                case OpCode.POP: {
                    stack.pop()
                    break
                }
                case OpCode.GETVAR: {
                    const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                    stack.push(BUILTIN_PROCS[varname] || frame.scope.get(varname))
                    break
                }
                case OpCode.DEFINEVAR: {
                    const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                    const val = stack.pop()
                    frame.scope.define(varname, val)
                    break
                }
                case OpCode.SETVAR: {
                    const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                    const val = stack.pop()
                    frame.scope.set(varname, val)
                    break
                }
                case OpCode.JUMPIFTRUE: {
                    const jumpIdx = frame.readNext()
                    if (isTruthy(stack.pop())) {
                        frame.ip = jumpIdx
                    }
                    break
                }
                case OpCode.JUMPIFFALSE: {
                    const jumpIdx = frame.readNext()
                    if (!isTruthy(stack.pop())) {
                        frame.ip = jumpIdx
                    }
                    break
                }
                case OpCode.JUMP: {
                    const jumpIdx = frame.readNext()
                    frame.ip = jumpIdx
                    break
                }

                case OpCode.CALL:
                case OpCode.TAILCALL: {
                    const isTail = opcode === OpCode.TAILCALL;
                    const nargs = frame.readNext();

                    // Extract out the target
                    const target = stack[stack.length - 1 - nargs];
                    stack.splice(stack.length - 1 - nargs, 1);
                    // Dispatch
                    this.#dispatchCall(target, nargs, isTail, frame, frames, stack, execScope);
                    break;
                }
                case OpCode.NEWCLOSURE: {
                    const template = frame.getConst(frame.readNext()) as ClosureTemplate
                    stack.push(new Closure(template, frame.scope));
                    break;
                }
                case OpCode.RETURN: {
                    frames.pop()
                    break
                }
                case OpCode.INTRINSIC_APPLY:
                case OpCode.INTRINSIC_TAIL_APPLY: {
                    const isTail = opcode === OpCode.INTRINSIC_TAIL_APPLY;
                    
                    const applyArgCount = stack.pop();
                    const listArg = stack.pop();
                    
                    const standardArgs = [];
                    for (let i = 0; i < applyArgCount - 1; i++) {
                        standardArgs.push(stack.pop());
                    }
                    standardArgs.reverse(); 
                    
                    // Extract out target
                    const target = stack.pop();
                    
                    // Flatten the final list
                    const finalArgs = [...standardArgs];
                    if (Array.isArray(listArg) || listArg instanceof Cons) {
                        finalArgs.push(...listArg);
                    } else if (listArg === null) {
                        // Empty list
                    } else {
                        throw new Error(`apply: last argument must be a list but got ${listArg}`);
                    }
                    
                    // Push flattened args to stack
                    for (const arg of finalArgs) {
                        stack.push(arg);
                    }

                    // Dispatch
                    this.#dispatchCall(target, finalArgs.length, isTail, frame, frames, stack, execScope);
                    break;
                }
                case OpCode.INTRINSIC_ADD: {
                    const nargs = stack.pop() as number;
                    let acc = 0; 
                    for (let i = 0; i < nargs; i++) {
                        const val = stack[stack.length - nargs + i]
                        if (typeof val !== "number") throw new Error(`+ requires numbers, but received ${typeof val}`);
                        acc += val
                    }
                    stack.length -= nargs
                    stack.push(acc)
                    break;
                }
                case OpCode.INTRINSIC_SUB: {
                    const nargs = stack.pop() as number;
                    if (nargs === 0) throw new Error("- requires at least 1 argument");
                    
                    if (nargs === 1) {
                        const val = stack[stack.length - 1];
                        if (typeof val !== "number") throw new Error(`- requires numbers, but received ${typeof val}`);
                        stack[stack.length - 1] = -val; 
                        break;
                    }

                    let acc = stack[stack.length - nargs];
                    if (typeof acc !== "number") throw new Error(`- requires numbers, but received ${typeof acc}`);
                    for (let i = 1; i < nargs; i++) {
                        const val = stack[stack.length - nargs + i]
                        if (typeof val !== "number") throw new Error(`- requires numbers, but received ${typeof val}`);
                        acc -= val
                    }
                    stack.length -= nargs
                    stack.push(acc)
                    break
                }
                case OpCode.INTRINSIC_MUL: {
                    const nargs = stack.pop() as number;
                    let acc = 1; 
                    for (let i = 0; i < nargs; i++) {
                        const val = stack[stack.length - nargs + i]
                        if (typeof val !== "number") throw new Error(`* requires numbers, but received ${typeof val}`);
                        acc *= val
                    }
                    stack.length -= nargs
                    stack.push(acc)
                    break;
                }
                case OpCode.INTRINSIC_DIV: {
                    const nargs = stack.pop() as number;
                    if (nargs === 0) throw new Error("/ requires at least 1 argument");
                    
                    if (nargs === 1) {
                        const val = stack[stack.length - 1];
                        if (typeof val !== "number") throw new Error(`/ requires numbers, but received ${typeof val}`);
                        if (val === 0) throw new Error("division by zero");
                        stack[stack.length - 1] = 1/val; 
                        break;
                    }

                    let acc = stack[stack.length - nargs];
                    if (typeof acc !== "number") throw new Error(`/ requires numbers, but received ${typeof acc}`);
                    for (let i = 1; i < nargs; i++) {
                        const val = stack[stack.length - nargs + i]
                        if (typeof val !== "number") throw new Error(`/ requires numbers, but received ${typeof val}`);
                        if (val === 0) throw new Error("/: division by zero")
                        acc /= val
                    }
                    stack.length -= nargs
                    stack.push(acc)
                    break
                }
                case OpCode.INTRINSIC_MODULO: {
                    const a = stack[stack.length-2]
                    const b = stack[stack.length-1]
                    if (typeof a !== "number" || typeof b !== "number") throw new Error(`modulo: requires numbers, but received ${typeof a}/${typeof b}`);
                    if (b === 0) throw new Error("modulo: division by zero");
                    stack.pop()
                    stack[stack.length-1] = ((a % b) + b) % b
                    break
                }
                case OpCode.INTRINSIC_REMAINDER: {
                    const a = stack[stack.length-2]
                    const b = stack[stack.length-1]
                    if (typeof a !== "number" || typeof b !== "number") throw new Error(`remainder: requires numbers, but received ${typeof a}/${typeof b}`);
                    if (b === 0) throw new Error("remainder: division by zero");
                    stack.pop()
                    stack[stack.length-1] = a % b
                    break
                }
                case OpCode.INTRINSIC_LIST: {
                    const nargs = stack.pop() as number;
                    const lst = stack.splice(stack.length - nargs, nargs);
                    stack.push(lst);
                    break
                }
                case OpCode.INTRINSIC_EQ: {
                    const nargs = stack.pop() as number;
                    if (nargs === 0) throw new Error("= requires at least 1 argument");
                    
                    let top = stack[stack.length - nargs];
                    if (typeof top !== "number") throw new Error(`= requires numbers, but received ${typeof top}`);
                    let res = true
                    for (let i = 1; i < nargs; i++) {
                        const val = stack[stack.length - nargs + i]
                        if (typeof val !== "number") throw new Error(`= requires numbers, but received ${typeof val}`);
                        if (val != top) {
                            res = false
                            break
                        }
                    }
                    stack.length -= nargs
                    stack.push(res)
                    break
                }
            }
        }
        
        if (stack.length > 1) {
            console.error(`Stack leak detected: ${stack.length} elems instead of 1, stack: ${stack}`)
        }
        if (stack.length > 0) return stack.pop()
        return undefined
    }

    /**
     * Note: stack must contain top narg elements in correct order (with arg0 at bottom and argN at top) with target already removed
     * @param target    The target procedure to execute (BuiltinFunction | NativeFunction | Closure).
     * @param nargs     The exact number of arguments currently sitting on top of the stack.
     * @param isTail    Whether to overwrite the current frame (TCO) instead of pushing a new one.
     * @param frame     The current active call frame.
     * @param frames    Reference to the global VM call stack.
     * @param stack     Reference to the global VM data/evaluation stack.
     * @param execScope VM global execution scope.
     */
    #dispatchCall(
        target: any, 
        nargs: number, 
        isTail: boolean, 
        frame: CallFrame, 
        frames: CallFrame[], 
        stack: any[], 
        execScope: AnimaScope
    ) {
        if (target instanceof BuiltinFunction) {
            if (target.nargs !== -1 && nargs !== target.nargs) {
                throw new Error(`Builtin expected ${target.nargs} args, got ${nargs}`);
            }
            
            if (target.nargs === -1) stack.push(nargs);

            const env = target.needsScope ? frame.scope : execScope;
            if (isTail) {
                frame.code = target.bc;
                frame.scope = env;
                frame.ip = 0; 
            } else {
                frames.push(new CallFrame(target.bc, env, 0));
            }
        } else if (target instanceof NativeFunction) {
            if (target.nargs !== -1 && target.nargs !== nargs) {
                throw new Error(`${target.name}: expected ${target.nargs} args, got ${nargs}`);
            }
            
            const args = nargs > 0 ? stack.splice(-nargs, nargs) : [];
            
            stack.push(target.cb(...args));
            if (isTail) frames.pop();
        } else if (target instanceof Closure) {
            const template = target.tmpl;
            const arity = template.params.length; // number of required args
            if (template.remParams !== null) {
                // variadic
                if (nargs < arity) {
                    throw new Error(`expected at least ${arity} args, got ${nargs}`);
                }
            } else {
                if (nargs !== arity) {
                    throw new Error(`expected exactly ${arity} args, got ${nargs}`);
                }
            }
    
            // Bind args
            const args = nargs > 0 ? stack.splice(-nargs, nargs) : [];
            const callScope = target.scope.nest();
            
            // required
            for (let i = 0; i < arity; i++) {
                callScope.define(template.params[i], args[i]);
            }
            if (template.remParams !== null) {
                // bind variadics
                const restArgs = args.slice(arity);
                callScope.define(template.remParams, restArgs);
            }

            if (isTail) {
                // Reuse existing frame (TCO)
                frame.code = template.code;
                frame.scope = callScope;
                frame.ip = 0; 
            } else {
                // Push new frame
                frames.push(new CallFrame(template.code, callScope, 0));
            }
        } else {
            throw new Error(`Attempted to call a non-procedure: ${String(target)}`);
        }
    }
}

// Optimizer layer

const FOLDABLE_MATH_OPS = new Set([
    OP_ADD,
    OP_SUB,
    OP_MUL,
    OP_DIV,
    OP_MODULO,
    OP_REMAINDER,
    OP_EQ,
])

class AnimaTransformer {
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
            
            // transform the inner first
            const expanded = ast.map(i => this.#transform(i));
            const expandedOp = expanded[0];

            switch (expandedOp) {
                case OP_COND:
                    return this.#transformCond(expanded)
                case OP_DEFINE:
                    return this.#transformDefineComplex(expanded)
                case OP_LET:
                    return this.#transformLet(expanded)
                case OP_LETSTAR:
                    return this.#transformLetStar(expanded)
                case OP_LETREC:
                    return this.#transformLetrec(expanded)
            }

            return expanded;
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

        return result; // Fixed
    }

    #transformDefineComplex(expr: any[]) {
        if(expr.length < 3) {
            throw new Error(`define must be in format ["define" varname arg] or [define (func_name arg1 arg2... argN) body_expr...] but have ${expr.length-1} arguments`)
        }

        if(typeof expr[1] === "symbol") {
            // Normal define
            if(expr.length !== 3) {
                throw new Error(`define must be in format (define varname expr), but received ${expr.length - 1} arguments`);
            }

            return expr
        } else if (Array.isArray(expr[1])) { 
            // (define (func_name arg1 arg2) body_expr...), this one just gets rewritten to a normal define with lambda
            if (expr[1].length === 0) throw new Error("define: missing function name");
            const funcName = expr[1][0];
            const params = expr[1].slice(1);
            const body = expr.slice(2);
            const equivExpr = [OP_DEFINE, funcName, [OP_LAMBDA, params, ...body]];
            return equivExpr
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
            return equivExpr
        } else {
            throw new Error(`define: ${String(expr[1])} not symbol or list syntax`)
        }
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
        const dummyVals: any[] = []; // The <undefined> initializers
        const setExprs: any[] = [];  // The (set! var expr) mutations

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

        // rewrite to lambda [((lambda (params...) (set! p1 v1)... body...) null...)]
        const equivExpr = [[OP_LAMBDA, params, ...setExprs, ...body], ...dummyVals];
        return equivExpr
    }
}

// Optimizes a fully transformed AST
class ASTOptimizer {
    public optimize(ast: any): any {
        // Base cases: primitives, strings, symbols, or null
        if (ast === null || typeof ast !== "object") {
            return ast;
        }

        if (ast instanceof DottedPair) {
            // optimize inner items of dotted pair
            ast.items = ast.items.map((item: any) => this.optimize(item));
            ast.rest = this.optimize(ast.rest);
            return ast;
        }

        if (Array.isArray(ast)) {
            if (ast.length === 0) return ast;

            const op = ast[0];

            if (op === OP_QUOTE) {
                return ast;
            }

            // Optimize all children first. This turns (+ 1 (* 2 3)) into (+ 1 6) for example, which we then optimize further to 7
            const optAst = ast.map(node => this.optimize(node));
            const optOp = optAst[0];

            // Prune if's
            if (optOp === OP_IF && optAst.length >= 3) {
                const condition = optAst[1];
                
                // If the condition is a resolved primitive (boolean, number, string)
                if (typeof condition === "boolean" || typeof condition === "number" || typeof condition === "string") {
                    const isTruthy = condition !== false;
                    
                    if (isTruthy) {
                        return optAst[2]; // Return the true branch
                    } else {
                        // Return the false branch or #<void>
                        return optAst.length > 3 ? optAst[3] : undefined;
                    }
                }
            }

            // Constant folding
            if (FOLDABLE_MATH_OPS.has(optOp)) {
                // Check if all arguments are literal numbers
                const isAllNumbers = optAst.length === 1 || optAst.slice(1).every(arg => typeof arg === "number");
                
                if (isAllNumbers) {
                    // try to do the math at compile time, if it fails, we know it wont work at runtime and 
                    // can just kill everything else
                    return this.#foldMath(optOp, optAst.slice(1));
                }
            }

            return optAst;
        }

        return ast;
    }

    // Tries to optimize math prior to passing to main compiler
    #foldMath(op: symbol, args: number[]): boolean | number {
        const nargs = args.length;

        if (op === OP_ADD) {
            return args.reduce((sum, val) => sum + val, 0);
        }

        if (op === OP_MUL) {
            return args.reduce((prod, val) => prod * val, 1);
        }

        if (op === OP_SUB) {
            if (nargs === 0) throw new Error("- requires at least 1 argument");
            if (nargs === 1) return -args[0];
            let acc = args[0];
            for (let i = 1; i < nargs; i++) acc -= args[i];
            return acc;
        }

        if (op === OP_DIV) {
            if (nargs === 0) throw new Error("/ requires at least 1 argument");
            if (nargs === 1) {
                if (args[0] === 0) throw new Error("/: division by zero");
                return 1 / args[0];
            }
            let acc = args[0];
            for (let i = 1; i < nargs; i++) {
                if (args[i] === 0) throw new Error("/: division by zero");
                acc /= args[i];
            }
            return acc;
        }

        if (op === OP_MODULO) {
            if (nargs !== 2) throw new Error("modulo requires 2 arguments");
            if (args[1] === 0) throw new Error("modulo: division by zero");
            return ((args[0] % args[1]) + args[1]) % args[1];
        }

        if (op === OP_REMAINDER) {
            if (nargs !== 2) throw new Error("modulo requires 2 arguments");
            if (args[1] === 0) throw new Error("modulo: division by zero");
            return ((args[0] % args[1]) + args[1]) % args[1];
        }

        if (op === OP_EQ) {
            if (nargs !== 0) throw new Error("= requires at least 1 argument");
            let top = args[0]
            for(let i = 1; i < args.length; i++) {
                if (args[i] != top) return false
            }
            return true
        }

        throw new Error("Unknown math op");
    }
}

class MapObj {
    #proc: any;
    #iters: (ArrayIterator<any> | Generator<any, void, unknown>)[]
    #isdone: boolean
    constructor(args: any[]) {
        if (args.length < 2) throw new Error("map requires at least 2 arguments (procedure and 1+ lists to map over)");
        
        const proc = args[0];
        const lists = args.slice(1)
        const iters = lists.map(list => {
            if (list === null) return [][Symbol.iterator]();
            if (Array.isArray(list) || list instanceof Cons) return list[Symbol.iterator]();
            throw new Error("map arguments must be lists");
        });
        this.#proc = proc
        this.#iters = iters
        this.#isdone = false
    }

    get done() { return this.#isdone }

    next() {
        if (this.#isdone) throw new Error("internal error: map iter done but %MapObjNext still called")
        const nextVals = this.#iters.map(it => it.next());
            
        if (nextVals.some(res => res.done)) {
            this.#isdone = true
            return
        }

        // Unlike cons, this avoids the allocation of O(N) extra cons array views making it more performant
        const args = [];
        for (const res of nextVals) {
            args.push(res.value);
        }

        return args
    }
}

export const PRELUDE_SCOPE = {
    [Symbol.for("%MapObj")]: new NativeFunction("%MapObj", -1, (...args) => {
        return new MapObj(args)
    }),
    [Symbol.for("%MapObjDone")]: new NativeFunction("%MapObjDone", 1, (...args) => {
        return args[0].done
    }),
    [Symbol.for("%MapObjNext")]: new NativeFunction("%MapObjNext", 1, (...args) => {
        return args[0].next()
    }),
    [Symbol.for("%ArrayNew")]: new NativeFunction("%ArrayNew", 0, (...args) => {
        return []
    }),
    [Symbol.for("%ArrayPush")]: new NativeFunction("%ArrayPush", 2, (...args) => {
        if(!Array.isArray(args[0])) throw new Error(`internal error: %ArrayPush called on non-array ${args[0]}`)
        args[0].push(args[1])
    }),
    [Symbol.for("%Builtin")]: new NativeFunction("%Builtin", 2, (...args) => {
        if (typeof args[0] !== "symbol") throw new Error(`${args[0]} is not a symbol`)
        if (!(args[1] instanceof Closure)) throw new Error(`${args[1]} is not a Closure`)
        console.log(`Registering ${String(args[0])}`)
        BUILTIN_PROCS[args[0]] = args[1]
    }),
    // @ts-ignore
    __proto__: null
}

// Prelude
const PRELUDE_VM = new AnimaVM()
PRELUDE_VM.evaluateStr(`
(%Builtin 'map (lambda (f . lists)
    (let ((iter (apply %MapObj f lists))
          (result (%ArrayNew)))
    
    (let loop ()
      (let ((args (%MapObjNext iter)))      
        (if (%MapObjDone iter)              
            result                          
            (begin
                (%ArrayPush result (apply f args)) 
                (loop))))))))    
`, PRELUDE_SCOPE)

/*
const TEST_PROG = `
(define union
    (lambda (a b)
        (define (in a rst) 
        (cond 
            [(empty? rst) #f]
            [(equal? a (car rst)) #t]
            [else (in a (cdr rst))]))

        (cond
        ; if either set is empty, the other one if the union
        [(empty? a) b]
        [(empty? b) a]
        ; if b is in a, skip it
        [(in (car b) a) (union a (cdr b))]
        [else (cons (car b) (union a (cdr b)))])))

(define sum-of-squares
  (lambda (a)
    ; do x*x for every element in a, then sum them all up
    (apply + [map (lambda (x) (* x x)) a])))
        
    (list (equal? (union '(a b d e f h j) '(f c e g a)) '(c g a b d e f h j)) (equal? (sum-of-squares (list 1 3 5 7)) 84))
`
//export const TEST_PROG = `(cond [#f 1] [#f 2])`

export const TEST_PROG_BC = new AnimaCompiler().compileExpr(new ASP(TEST_PROG).parse(), false, false)
*/