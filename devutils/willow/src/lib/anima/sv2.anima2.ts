import { ASP, BUILTIN_PROCS, OP_AND, OP_BEGIN, OP_COND, OP_DEFINE, OP_ELSE, OP_IF, OP_LAMBDA, OP_LET, OP_OR, OP_QUOTE, SPECIAL_FORMS } from "./sv2.anima";

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
    // Tail call a builtin or custom procedure (TAILCALL nargs). This reuses the existing stack frame instead of creating a new one (like call does)
    TAILCALL,
    // Return from function with top value as return value. All other values are cleared from stack       
    RETURN,
    // Creates a Closure out of a ClosureTemplate (NEWCLOSURE idx) and pushes it to top of stack
    NEWCLOSURE,

    // Intrinsics
    INTRINSIC_APPLY, // apply ()
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
            const idx = this.constants.push(v) - 1
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
}

export class AnimaCompiler {
    compileExpr(expr: any[], disableDefine: boolean, disableLambda: boolean) {
        const bc = new ByteCode();
        this.#compile(expr, {leaveOnStack: true, isTail: true, bc, tryOpt: true, disableDefine, disableLambda })
        return bc
    }
    #compile(expr: any, opts: CmpOpts) {
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
                    this.#compileBegin(expr, opts)
                    return
                case OP_IF:
                    this.#compileIfCall(expr, opts)
                    return
                case OP_COND:
                    this.#compileCond(expr, opts)
                    return
                case OP_QUOTE:
                    this.#compileQuote(expr, opts)
                    return
                case OP_DEFINE:
                    this.#compileDefine(expr, opts)
                    return
                case OP_LAMBDA:
                    this.#compileLambda(expr, opts)
                    return
                case OP_LET:
                    this.#compileLet(expr, opts)
                    return
                case OP_AND:
                    this.#compileAnd(expr, opts)
                    return
                case OP_OR:
                    this.#compileOr(expr, opts)
                    return
            }
        }

        this.#compileNormalCall(expr, opts)
    }

    #compileBegin(expr: any[], opts: CmpOpts) {
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
    #compileNormalCall(expr: any[], opts: CmpOpts) {
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
            if (!opts.leaveOnStack) opts.bc.pop()
        }
    }

    // compiles both if calls as well as code that is converted into if calls
    #compileIfCall(expr: any[], opts: CmpOpts) {
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

    #compileCond(expr: any[], opts: CmpOpts) {
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

    #compileQuote(expr: any[], opts: CmpOpts) {
        if(expr.length != 2) {
            throw new Error(`quote must be in format ["quote", expr] but have ${expr.length-1} arguments`)
        }
        if(!opts.leaveOnStack) return
        opts.bc.push(expr[1])
    }

    #compileDefine(expr: any[], opts: CmpOpts) {
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
            if (expr[1] in BUILTIN_PROCS) {
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

    #compileLambda(expr: any[], opts: CmpOpts) {
        if (!opts.leaveOnStack) return
        if(opts.disableLambda) {
            throw new Error("lambda expressions are disabled in this context")
        }

        if(expr.length < 3) {
            throw new Error(`lambda must be in format ["lambda", [bind-args...], body...] but only have ${expr.length-1} arguments`)
        }

        if (!Array.isArray(expr[1])) throw new Error("lambda arguments must be a list");
        // Validate that every parameter is a symbol
        for(let i = 0; i < expr[1].length; i++) {
            if(typeof expr[1][i] !== "symbol") {
                throw new Error(`lambda parameter at index ${i} must be a symbol, but received ${typeof expr[1][i]}: ${String(expr[1][i])}`);
            }
            if (SPECIAL_FORMS.has(expr[1][i])) {
                throw new Error(`${String(expr[1][i])}: bad syntax`)
            }
            if (expr[1][i] in BUILTIN_PROCS) {
                throw new Error(`${String(expr[1][i])}: cannot shadow builtin procedure`)
            }
        }

        // Compile lambda body
        const lambdaBc = new ByteCode()
        this.#compile(this.#wrapMulti(expr.slice(2)), {...opts, leaveOnStack: true, isTail: true, bc: lambdaBc})
        lambdaBc.return()
        const template = new ClosureTemplate(expr[1], lambdaBc);
        opts.bc.newclosure(template)
    }

    // TODO: Support named let form, maybe let*/letrec as well later
    #compileLet(expr: any[], opts: CmpOpts) {
        // OP_LET is special in that it gets compiled down to a lambda in the end

        // normal let: (let ((var expr) ...) body1 body2 ...)
        if (expr.length < 3) throw new Error(`let must be in format ["let", [[var expr]...], body...] but only have ${expr.length-1} arguments`);

        const bindings = expr[1];
        if (!Array.isArray(bindings) && bindings !== null) {
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
                } else {
                    throw new Error(`let binding \`${binding}\` must be a list of form [var expr]`);
                }

                if (typeof sym !== "symbol") throw new Error("let binding name must be a symbol");
                
                params.push(sym);
                exprs.push(val);
            }
        }

        // rewrite to lambda [(let ((var expr) ...) body1 body2 ...) => ((lambda (var...) body1 body2...) expr...)]
        const equivExpr = [[OP_LAMBDA, params, ...body], ...exprs];
        this.#compile(equivExpr, opts)
    }

    #compileAnd(expr: any[], opts: CmpOpts) {
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

    #compileOr(expr: any[], opts: CmpOpts) {
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
}

/** A template for a closure that can then be bound to a scope */
export class ClosureTemplate {
    params: symbol[];
    code: ByteCode

    constructor(params: symbol[], code: ByteCode) {
        this.params = params
        this.code = code
    }
}

/*const TEST_PROG = `
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
`*/
export const TEST_PROG = `(cond [#f 1] [#f 2])`

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
    /*
    case OpCode.CALL: {
    const argCount = inst[frame.ip++];
    const target = stack[stack.length - 1 - argCount]; // Get target w/o popping

    if (target instanceof BuiltinFunction) {
        // Pop target out
        stack.splice(stack.length - 1 - argCount, 1);

        if (target.nargs !== -1 && argCount !== target.nargs) {
            throw new Error(`Builtin expected ${target.nargs} arguments, got ${argCount}`);
        }

        // If variadic, push argCount so builtin functions know many arguments it has
        if (target.nargs === -1) {
            stack.push(argCount);
        }
        const env = target.needsScope ? frame.env : globalExecutionScope;
        frames.push(new CallFrame(target.bc.inst, env));
        break; 
    }
    */
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
    bc.intrinsicApply()
});