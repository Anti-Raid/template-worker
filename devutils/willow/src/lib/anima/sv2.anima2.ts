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
  isTruthy
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
        } else if(Array.isArray(v) && v.length === 0) {
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
    disableLambda: boolean,
}

const SYMBOL_DOT = Symbol.for(".")

export class AnimaCompiler {
    compileExpr(expr: any[], disableDefine: boolean, disableLambda: boolean) {
        const bc = new ByteCode();
        this.#compile(expr, {leaveOnStack: true, isTail: true, bc, tryOpt: true, disableDefine, disableLambda })
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
                    this.#compileCond(expr, opts, syntaxCtx)
                    return
                case OP_QUOTE:
                    this.#compileQuote(expr, opts, syntaxCtx)
                    return
                case OP_DEFINE:
                    this.#compileDefine(expr, opts, syntaxCtx)
                    return
                case OP_LAMBDA:
                    this.#compileLambda(expr, opts, syntaxCtx)
                    return
                case OP_LET:
                    this.#compileLet(expr, opts, syntaxCtx)
                    return
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
            }
        }

        this.#compileNormalCall(expr, opts)
    }

    #compileBegin(expr: any, opts: CmpOpts, syntaxCtx?: string) {
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
    #compileNormalCall(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        // We need to compile the first arg first and leave it on the stack
        //
        // This will place a e.g. (PUSH) <symbol>
        this.#compile(expr[0], { ...opts, leaveOnStack: true, isTail: false })

        // TODO: Do param number here (maybe???)

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
    #compileIfCall(expr: any, opts: CmpOpts, syntaxCtx?: string) {
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

    #compileCond(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        // this just desugars down to a bunch of ifs
        if (expr.length === 1) throw new Error("cond requires at least one clause");

        let result: any = []; 

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

        this.#compile(result, opts);
    }

    #compileQuote(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        if(expr.length != 2) {
            throw new Error(`quote must be in format ["quote", expr] but have ${expr.length-1} arguments`)
        }
        if(!opts.leaveOnStack) return
        opts.bc.push(expr[1])
    }

    #compileDefine(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        if (opts.disableDefine) {
            throw new Error("define expressions are disabled in this context");
        }

        if(expr.length < 3) {
            throw new Error(`define must be in format ["define" varname arg] or [define (func_name arg1 arg2... argN) body_expr...] but have ${expr.length-1} arguments`)
        }

        // Normal define
        if(typeof expr[1] === "symbol") {
            if (SPECIAL_FORMS.has(expr[1])) {
                throw new Error(`${String(expr[1])}: bad syntax`)
            }
            if (expr[1] in BUILTINS_OPS) {
                throw new Error(`${String(expr[1])}: cannot shadow builtin procedure`)
            }
            // We need to compile the second arg first and leave it on the stack
            //
            // This will place a e.g. (PUSH) <symbol>
            this.#compile(expr[2], { ...opts, leaveOnStack: true, isTail: false });
            opts.bc.defineVar(expr[1])
            if (opts.leaveOnStack) {
                opts.bc.push(undefined);
            }
        } else if (Array.isArray(expr[1])) { 
            // (define (func_name arg1 arg2) body_expr...), this one just gets rewritten to a normal define with lambda
            if (expr[1].length === 0) throw new Error("define: missing function name");
            const funcName = expr[1][0];
            const params = expr[1].slice(1);
            const body = expr.slice(2);
            const equivExpr = [OP_DEFINE, funcName, [OP_LAMBDA, params, ...body]];
            this.#compile(equivExpr, opts)
        } else {
            throw new Error(`${String(expr[1])}: expr[1] not symbol or list syntax`)
        }
    }

    #compileLambda(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        if (!opts.leaveOnStack) return
        if(opts.disableLambda) {
            throw new Error(`${syntaxCtx || "lambda"} expressions are disabled in this context [when compiling a lambda]`)
        }

        if(expr.length < 3) {
            throw new Error(`lambda must be in format ["lambda", [bind-args...], body...] but only have ${expr.length-1} arguments`)
        }

        const params: symbol[] = []
        let remParams: symbol | null = null
        let foundDot = false
        if (Array.isArray(expr[1])) {
            // Validate that every parameter is a symbol
            const seen = new Set<symbol>();
            for(let i = 0; i < expr[1].length; i++) {
                if(typeof expr[1][i] !== "symbol") {
                    throw new Error(`${syntaxCtx || "lambda"} parameter at index ${i} must be a symbol, but received ${typeof expr[1][i]}: ${String(expr[1][i])}`);
                }
                if (expr[1][i] === SYMBOL_DOT) {
                    if (foundDot || remParams) throw new Error(`illegal use of \`.\` (multiple . is not allowed)`)
                    foundDot = true
                    continue
                }
                if (seen.has(expr[1][i])) {
                    throw new Error(`${syntaxCtx || "lambda"} parameter at index ${i} is a duplicate parameter name: ${String(expr[1][i])}`);
                }
                seen.add(expr[1][i])

                if (SPECIAL_FORMS.has(expr[1][i])) {
                    throw new Error(`${String(expr[1][i])}: bad syntax`)
                }
                if (expr[1][i] in BUILTINS_OPS) {
                    throw new Error(`${String(expr[1][i])}: cannot shadow builtin procedure`)
                }

                if (!foundDot) params.push(expr[1][i])
                else {
                    if (remParams) throw new Error(`illegal use of \`.\` (more than one symbol after dot)`); 
                    remParams = expr[1][i]
                }
            }

            if (foundDot && remParams === null) {
                throw new Error(`illegal use of \`.\` (trailing . is not allowed)`)
            }
        } else if (typeof expr[1] === "symbol") {
            // Then all args must be bound to remparams
            remParams = expr[1]
        } else {
            throw new Error(`${syntaxCtx || "lambda"} arguments must be a symbol (to bind all as a list to said symbol) or a list`);
        }

        // Compile lambda body
        const lambdaBc = new ByteCode()
        this.#compile(this.#wrapMulti(expr.slice(2)), {...opts, leaveOnStack: true, isTail: true, bc: lambdaBc})
        lambdaBc.return()
        const template = new ClosureTemplate(params, remParams, lambdaBc);
        opts.bc.newclosure(template)
    }

    // TODO: Support named let form, maybe let*/letrec as well later
    #compileLet(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        // OP_LET is special in that it gets compiled down to a lambda in the end
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
            this.#compile(namedLetExpr, opts, "named let");
        } else {
            // rewrite to lambda [(let ((var expr) ...) body1 body2 ...) => ((lambda (var...) body1 body2...) expr...)]
            const equivExpr = [[OP_LAMBDA, params, ...body], ...exprs];
            this.#compile(equivExpr, opts, "let");
        }
    }

    #compileAnd(expr: any, opts: CmpOpts, syntaxCtx?: string) {
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

    #compileOr(expr: any, opts: CmpOpts, syntaxCtx?: string) {
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
    #optApply(expr: any, opts: CmpOpts, syntaxCtx?: string) {
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
export class Closure {
    constructor(public tmpl: ClosureTemplate, public scope: AnimaScope) {}
}

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

/** 
 * A builtin function. Builtin functions do not have access to their own lexical scope (at least not yet) 
 * 
 * 
 * Unlike normal functions which pop from stack and bind to a new AnimaScope, builtin funcs keep values on
 * stack and just do bytecode replacement
*/
export class BuiltinFunction {
    // number of args needed on stack top, -1 means variadic
    // if we are in variadic mode, the top of stack will contain 
    // the number of arguments pushed on the stack
    //
    // Logic is similar to:
    nargs: number 
    bc: ByteCode
    needsScope: boolean // do we need scope or not (if false, this will use the global execution scope as the scope in bytecode)

    constructor(nargs: number, needsScope: boolean, initializer: (bc: ByteCode) => void) {
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

export class NativeFunction {
    constructor(
        public name: string,
        public nargs: number, // -1 for variadic
        public cb: (...args: any[]) => any
    ) {}
}

const BUILTIN_PROCS: Record<symbol, BuiltinFunction | NativeFunction> = {
    [OP_APPLY]: BUILTINS_APPLY
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
    constructor(public maxSteps: number) {}

    public evaluate(code: ByteCode, rawData: Record<string, any>): any {
        const globalScope = new AnimaScope(rawData, null, {steps: 0})
        const executionScope = globalScope.nest(); // Any "define" calls now write to this temporary scope
        return this.#evalinner(code, executionScope);
    }

    public evaluateExpr(expr: any, disableDefine: boolean, disableLambda: boolean, rawData: Record<string, any>): any {
        const bc = (new AnimaCompiler()).compileExpr(expr, disableDefine, disableLambda)
        return this.evaluate(bc, rawData);
    }

    #evalinner(code: ByteCode, execScope: AnimaScope): any {
        let scopeState = execScope.state

        // Initial frame and stack
        let frames: CallFrame[] = [new CallFrame(code, execScope, 0)];
        let stack: any[] = [];

        while (frames.length > 0) {
            scopeState.steps++;
            if (this.maxSteps && scopeState.steps > this.maxSteps) {
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
                        throw new Error("apply: last argument must be a list");
                    }
                    
                    // Push flattened args to stack
                    for (const arg of finalArgs) {
                        stack.push(arg);
                    }

                    // Dispatch
                    this.#dispatchCall(target, finalArgs.length, isTail, frame, frames, stack, execScope);
                    break;
                }
            }
        }
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