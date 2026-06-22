import { BUILTIN_PROCS, OP_BEGIN, OP_COND, OP_DEFINE, OP_ELSE, OP_IF, OP_LAMBDA, OP_QUOTE, SPECIAL_FORMS } from "./sv2.anima";

export enum OpCode {
    // Push a constant number to the stack
    PUSHNUMBER,
    // Push a constant boolean (1/0) to the stack (PUSHBOOLEAN 0/1)
    PUSHBOOLEAN,
    // Push a empty list to the stack
    PUSHEMPTYLIST,
    // Push the void element (undefined) to the stack
    PUSHVOID,
    // Push a constant from consts to the stack
    PUSH, 
    // Pops out the top argument of the stack
    POP,
    // Get a variable from either the list of registered builtins or the current scope (GETVAR [varname-idx])
    GETVAR,
    // Set the top stack value on the stack on the current scope (SETVAR [varname-idx])
    DEFINEVAR,
    // Jump if stack top is false
    JUMPIFTRUE,
    // Jump if stack top is false
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
}

export class ByteCode {
    #knownSymbols: Record<symbol, number>;
    constructor(public constants: any[], public inst: number[]) {
        this.#knownSymbols = Object.create(null)
    }

    pushNumber = (v: number) => this.inst.push(OpCode.PUSHNUMBER, v)
    pushBoolean = (v: boolean) => this.inst.push(OpCode.PUSHBOOLEAN, v ? 1 : 0)
    pushEmptyList = () => this.inst.push(OpCode.PUSHEMPTYLIST)
    pushVoid = () => this.inst.push(OpCode.PUSHVOID)
    push = (v: any) => {
        if (typeof v === "symbol") {
            if(v in this.#knownSymbols) {
                this.inst.push(OpCode.PUSH, this.#knownSymbols[v])
            } else {
                const idx = this.constants.push(v) - 1
                this.#knownSymbols[v] = idx
                this.inst.push(OpCode.PUSH, idx)
            }
        } else {
            // TODO: Deduplicate stuff later once this actually works
            const idx = this.constants.push(v) - 1
            this.inst.push(OpCode.PUSH, idx)
        }
    }
    pop = () => this.inst.push(OpCode.POP)
    getVar = (varname: symbol) => {
        if(varname in this.#knownSymbols) {
            this.inst.push(OpCode.GETVAR, this.#knownSymbols[varname])
        } else {
            const idx = this.constants.push(varname) - 1
            this.#knownSymbols[varname] = idx
            this.inst.push(OpCode.GETVAR, idx)
        }
    }
    defineVar = (varname: symbol) => {
        if(varname in this.#knownSymbols) {
            this.inst.push(OpCode.DEFINEVAR, this.#knownSymbols[varname])
        } else {
            const idx = this.constants.push(varname) - 1
            this.#knownSymbols[varname] = idx
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

    call = (args: number) => this.inst.push(OpCode.CALL, args)
    tailcall = (args: number) => this.inst.push(OpCode.TAILCALL, args)
    return = () => this.inst.push(OpCode.RETURN)
    newclosure = (tmplInfo: ClosureTemplate) => {
        const idx = this.constants.push(tmplInfo) - 1
        this.inst.push(OpCode.NEWCLOSURE, idx)
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
        const bc = new ByteCode([], []);
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
        } else if (typeof expr === "boolean") {
            if (!opts.leaveOnStack && opts.tryOpt) return 
            opts.bc.pushBoolean(expr)
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
            opts.bc.pushEmptyList()
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
            }
        }

        this.#compileNormalCall(expr, opts)
    }

    #compileBegin(expr: any[], opts: CmpOpts) {
        // We need to push a void if we see an empty begin block
        if (expr.length === 1) {
            if (opts.leaveOnStack) opts.bc.pushVoid();
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
                opts.bc.pushVoid();
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
        const lambdaBc = new ByteCode([], [])
        this.#compile(this.#wrapMulti(expr.slice(2)), {...opts, leaveOnStack: true, isTail: true, bc: lambdaBc})
        lambdaBc.return()
        const template = new ClosureTemplate(expr[1], lambdaBc);
        opts.bc.newclosure(template)
    }
}

/** JS Closure */
export class ClosureTemplate {
    params: symbol[];
    code: ByteCode

    constructor(params: symbol[], code: ByteCode) {
        this.params = params
        this.code = code
    }
}
