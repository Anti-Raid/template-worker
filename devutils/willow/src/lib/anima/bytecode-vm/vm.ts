import { AnimaScope, ExposedProps, IProcedure } from "../common";
import { isTruthy } from "../common";
import { Cons } from "../list";
import { AnimaCompiler } from "./compiler";

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

    // ui-get (1 arg, no nargs)
    INTRINSIC_UI_GET
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
                case OpCode.INTRINSIC_UI_GET:
                    line += `INTRINSIC_UI_GET`
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

export class NativeFunction extends IProcedure {
    constructor(
        public name: string,
        public nargs: number, // -1 for variadic
        public cb: (...args: any[]) => any
    ) {
        super()
    }
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

    public evaluate(code: ByteCode, scope: AnimaScope, props?: ExposedProps): any {
        return this.#evalinner(code, scope, props);
    }

    public evaluateExpr(expr: any, disableDefine: boolean, disableLambda: boolean, scope: AnimaScope, props?: ExposedProps): any {
        const bc = (new AnimaCompiler()).compileExpr(expr, disableDefine, disableLambda)
        return this.evaluate(bc, scope, props);
    }

    public evaluateStr(expr: string, scope: AnimaScope, disableDefine: boolean = false, disableLambda: boolean = false, props?: ExposedProps): any {
        const bc = (new AnimaCompiler()).compileStr(expr, disableDefine, disableLambda)
        return this.evaluate(bc, scope, props);
    }

    #evalinner(code: ByteCode, execScope: AnimaScope, props?: ExposedProps): any {
        // Initial frame and stack
        let frames: CallFrame[] = [new CallFrame(code, execScope, 0)];
        let stack: any[] = [];

        while (frames.length > 0) {
            this.steps++;
            if (this.maxSteps && this.steps > this.maxSteps) {
                throw new Error(`Script ran for more than ${this.maxSteps} instructions.`);
            }

            const frame = frames[frames.length - 1];

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
                    stack.push(frame.scope.get(varname))
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
                case OpCode.INTRINSIC_UI_GET: {
                    let varname = stack.pop()
                    if (typeof varname !== "symbol") throw new Error(`ui-get expected symbol, but received ${typeof varname}`);
                    if (props === undefined) {
                        stack.push(undefined)
                    } else {
                        const prop = Symbol.keyFor(varname) || varname.description
                        if (!prop) throw new Error(`internal error: ui-get expected string-able symbol but symbol not stringable`)
                        stack.push(props.get(prop))
                    }
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