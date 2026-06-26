import {
  OP_DEFINE,
  OP_BEGIN,
  OP_LAMBDA,
  OP_LET,
  OP_IF,
  OP_COND,
  OP_QUOTE,
  OP_AND,
  OP_OR,
  OP_LIST,
  OP_APPLY,
  OP_EQ,
  OP_ADD,
  OP_SUB,
  OP_MUL,
  OP_DIV,
  OP_MODULO,
  SPECIAL_FORMS,
  BUILTINS_OPS,
  ASP,
  OP_SET,
  OP_LETREC,
  OP_LETSTAR,
  DottedPair,
  ASTStringifier,
  OP_REMAINDER,
  OP_UI_GET
} from "../common";
import { Cons } from "../list";
import { AnimaOptimizer } from "../optimizer";
import { AnimaTransformer } from "../syntax-transformer";
import { ClosureTemplate, OpCode, ByteCode, type UpVarLoc } from "./vm";

/** A helper to create bytecode */
class ByteCodeBuilder {
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
    getVar(varname: symbol, scope: CompilerScope) {
        // Check if we can resolve it to a local/upvar
        const resolved = scope.resolve(varname)

        if (resolved.type === 'Local') {
            this.inst.push(OpCode.GETLOCAL, resolved.index);
            return;
        } 
        
        if (resolved.type === 'Upvar') {
            this.inst.push(OpCode.GETUPVAR, resolved.index);
            return;
        }

        // Handle it as a global
        const symIdx = this.#knownSymbols.get(varname)
        if(symIdx) {
            this.inst.push(OpCode.GETGLOBALS, symIdx)
        } else {
            const idx = this.constants.push(varname) - 1
            this.#knownSymbols.set(varname, idx)
            this.inst.push(OpCode.GETGLOBALS, idx)
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
    setVar(varname: symbol, scope: CompilerScope) {
        // Check if we can resolve it to a local/upvar
        const resolved = scope.resolve(varname)

        if (resolved.type === 'Local') {
            this.inst.push(OpCode.SETLOCAL, resolved.index);
            return;
        } 
        
        if (resolved.type === 'Upvar') {
            this.inst.push(OpCode.SETUPVAR, resolved.index);
            return;
        }

        // Handle it as a global
        const symIdx = this.#knownSymbols.get(varname)
        if(symIdx) {
            this.inst.push(OpCode.SETGLOBALS, symIdx)
        } else {
            const idx = this.constants.push(varname) - 1
            this.#knownSymbols.set(varname, idx)
            this.inst.push(OpCode.SETGLOBALS, idx)
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

    intrinsicUiGet() {
        this.inst.push(OpCode.INTRINSIC_UI_GET)
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

    build(numLocals: number) {
        return new ByteCode(this.constants, this.inst, numLocals)
    }
}

type Resolve = { type: "Global" } | { type: "Local", index: number } | { type: "Upvar", index: number }

/** 
 * Tracks/'simulates' block-level variable shadowing (within a function) at compile-time 
 * 
 * Used internally for optimizing out IIFE's etc.
*/
class Block {
    bindings = new Map<symbol, number>();
    parent: Block | null;

    constructor(parent: Block | null = null) {
        this.parent = parent;
    }

    // Walks up the nested blocks (within the SAME function) to find the slot
    resolve(sym: symbol): number | null {
        if (this.bindings.has(sym)) return this.bindings.get(sym)!;
        if (this.parent) return this.parent.resolve(sym);
        return null;
    }
}

/** Helper utility for keeping track of variable scoping */
class CompilerScope {
    // Keeps track of variables that have been shadowed etc.
    currBlock: Block = new Block();
    currSlot: number = 0;

    outer: CompilerScope | null;
    upvars: UpVarLoc[] = [];

    constructor(outer: CompilerScope | null) {
        this.outer = outer;
        this.upvars = []
    }

    get numLocals() {
        return this.currSlot
    }

    enterBlock() {
        this.currBlock = new Block(this.currBlock);
    }

    exitBlock() {
        if (this.currBlock.parent) {
            this.currBlock = this.currBlock.parent;
        } else {
            throw new Error("internal error: cannot exit root block of CompilerScope.");
        }
    }
    
    // Returns the index the variable is defined at
    addLocal(sym: symbol) {
        const slot = this.currSlot++;
        this.currBlock.bindings.set(sym, slot);
        return slot;
    }

    // Returns the result of resolving
    resolve(sym: symbol): Resolve {
        // Check if its a local
        const index = this.currBlock.resolve(sym)
        if (index !== null) return { type: 'Local', index }
        // Check if its global
        if (!this.outer) return { type: "Global" }
        
        // Ask parent to try resolving it as a upvar
        const parentResolved = this.outer.resolve(sym)
        if (parentResolved.type === 'Local') {
            return { 
                type: 'Upvar', 
                index: this.#recordUpvar({ local: true, index: parentResolved.index }) 
            };
        } 
    
        if (parentResolved.type === 'Upvar') {
            return { 
                type: 'Upvar', 
                index: this.#recordUpvar({ local: false, index: parentResolved.index }) 
            };
        }

        return parentResolved // global
    }

    // Records a upvar from parent scope
    #recordUpvar(upvar: UpVarLoc) {
        // Check if we already captured this exact upvalue to avoid duplicates
        const existingIdx = this.upvars.findIndex(u => u.index === upvar.index && u.local === upvar.local);
        if (existingIdx !== -1) {
            //console.log("recorded upvar", upvar, "at index:", existingIdx);
            return existingIdx;
        }
        return this.upvars.push(upvar) - 1;
    }
}

interface CmpOpts {
    leaveOnStack: boolean // whether to leave created values on the stack or not
    isTail: boolean // whether this is a tail-call or not (for tco)
    bc: ByteCodeBuilder
    tryOpt: boolean
    disableDefine: boolean
    disableLambda: boolean
    disableSet: boolean
    scope: CompilerScope
}

export class AnimaCompiler {
    s = new ASTStringifier()
    t = new AnimaTransformer()
    o = new AnimaOptimizer()

    compileStr(s: string, disableDefine: boolean = false, disableLambda: boolean = false, disableSet: boolean = false, tryOpt = true) {
        return this.compileExpr(new ASP(s, true).parse(), disableDefine, disableLambda, disableSet, tryOpt)
    }

    compileExpr(expr: any, disableDefine: boolean = false, disableLambda: boolean = false, disableSet: boolean = false, tryOpt = true) {
        // First transform the expr so all conds are resolved
        let trExpr = this.t.transform(expr)
        if (tryOpt) trExpr = this.o.optimize(trExpr)

        const bc = new ByteCodeBuilder();
        const scope = new CompilerScope(null)
        this.#compile(trExpr, {leaveOnStack: true, isTail: true, bc, tryOpt, disableDefine, disableLambda, disableSet, scope })
        return bc.build(scope.numLocals)
    }

    #compile(expr: any, opts: CmpOpts, syntaxCtx?: string) {
        // Raw values
        if (typeof expr === 'symbol') {
            opts.bc.getVar(expr, opts.scope)
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
                case OP_LETSTAR:
                case OP_LETREC:
                    throw new Error("internal error: let should be transformed by AnimaTransform prior to reaching here")
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
                case OP_UI_GET:
                    this.#optIntrinsicOneArgs(expr, opts, syntaxCtx)
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
        // Try IIFE optimizations
        if(this.#optIIFE(expr, opts)) {
            return
        }

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

    // note that the syntax transformer alr handles defines inside a lambda so
    #compileDefine(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        if (opts.scope.outer) {
            throw new Error(`internal error: define found inside lambda-expr or other lexical scoping (let/let*/letrec), internal defines should be transformed by AnimaTransform prior to reaching here`);
        }

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
        opts.bc.setVar(expr[1], opts.scope)
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

        const lambdaScope = new CompilerScope(opts.scope)

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
            lambdaScope.addLocal(params[i])
        }
        if (remParams) {
            this.#ensureCanBind(remParams, seen, syntaxCtx || "lambda")
            lambdaScope.addLocal(remParams)
        }

        // Once we've verified the syntax, we can then drop the entire lambda if its not actually needed on the stack
        //
        // This lets us keep the syntax checking without the work
        if (!opts.leaveOnStack) return

        // Compile lambda body
        const lambdaBc = new ByteCodeBuilder()
        this.#compile(this.#wrapMulti(expr.slice(2)), {...opts, leaveOnStack: true, isTail: true, bc: lambdaBc, scope: lambdaScope })
        lambdaBc.return()
        const template = new ClosureTemplate(params, remParams, lambdaBc.build(lambdaScope.numLocals), lambdaScope.upvars);
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
        else if (op === OP_REMAINDER) opts.bc.intrinsicRemainder();
        else throw new Error(`internal error: no intrinsic for op ${op}`)
        
        if (!opts.leaveOnStack) {
            opts.bc.pop();
        }
    }

    #optIntrinsicOneArgs(expr: any[], opts: CmpOpts, syntaxCtx?: string) {
        const op = expr[0]

        if (expr.length !== 2) {
            throw new Error(`${op} requires exactly 1 arguments, got ${expr.length - 1}`);
        }

        this.#compile(expr[1], { ...opts, leaveOnStack: true, isTail: false });
        
        if (op === OP_UI_GET) opts.bc.intrinsicUiGet();
        else throw new Error(`internal error: no intrinsic for op ${op}`)
        
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

    #optIIFE(expr: any[], opts: CmpOpts, syntaxCtx?: string): boolean {
        // if we have a non-variadic IIFE ((lambda (params...) body) args...), then we can optimize it down
        // to BLOCK/ENDBLOCK instead of doing a whole function call
        //
        // TODO: add a thing to transformer etc to track down call/cc calls so we can bail out if we see a call/cc
        // call. Not important right now though as we don't support call/cc
        const first = expr[0]
        if (Array.isArray(first) && first[0] === OP_LAMBDA && Array.isArray(first[1])) {
            const params = first[1];
            const body = first.slice(2);
            const args = expr.slice(1)
            if (params.length !== args.length) {
                throw new Error(`expected exactly ${params.length} args, got ${args.length}`);
            }
            
            // Bind all arguments in prior to entering the iife's block
            const seen = new Set<symbol>();
            for(let i = 0; i < params.length; i++) {
                this.#ensureCanBind(params[i], seen, syntaxCtx || "lambda")
                this.#compile(args[i], { ...opts, leaveOnStack: true, isTail: false })
            }

            opts.scope.enterBlock()
            // alloc slots for every parameter
            for(let i = 0; i < params.length; i++) {
                opts.scope.addLocal(params[i])
            }
            // setlocal all of our args in reverse order
            for(let i = params.length - 1; i >= 0; i--) {
                opts.bc.setVar(params[i], opts.scope) // note to self: setVar calls resolve internally which will then yield the exact slot
            }

            this.#compile(this.#wrapMulti(body), opts)
            opts.scope.exitBlock()
            return true
        } else {
            return false
        }
    }
}