import { ExposedProps, IProcedure, MissingVarError } from "../common";
import { isTruthy } from "../common";
import { Cons } from "../list";

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

    // Gets/sets variables
    GETUPVAR, // <upvar idx>
    SETUPVAR, // <upvar idx>, value on top of stack, *always* pops stack top
    GETLOCAL, // <slot>
    SETLOCAL, // <slot>, value on top of stack, *always* pops stack top
    GETGLOBALS, // <symbol>
    SETGLOBALS, // <symbol>, value on top of stack, *always* pops stack top
    // defines the top stack value on the stack on the current scope (DEFINEVAR [varname-idx]), *always* pops stack top
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

export class ByteCode {
    public constants: any[]
    public inst: Uint32Array
    public numLocals: number
    constructor(constants: any[], inst: number[], numLocals: number) {
        this.constants = constants
        this.inst = new Uint32Array(inst)
        this.numLocals = numLocals
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
            return `(${r.join(' ')})`
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
                case OpCode.GETUPVAR:
                    line += `GETUPVAR (slot=${this.inst[idx + 1]})`;
                    idx += 2;
                    break;
                case OpCode.GETLOCAL:
                    line += `GETLOCAL (slot=${this.inst[idx + 1]})`;
                    idx += 2;
                    break;
                case OpCode.GETGLOBALS:
                    line += `GETGLOBALS ${this.#constToString(this.constants[this.inst[idx + 1]])}`;
                    idx += 2;
                    break;
                case OpCode.SETUPVAR:
                    line += `SETUPVAR (slot=${this.inst[idx + 1]})`;
                    idx += 2;
                    break;
                case OpCode.SETLOCAL:
                    line += `SETLOCAL (slot=${this.inst[idx + 1]})`;
                    idx += 2;
                    break;
                case OpCode.SETGLOBALS:
                    line += `SETGLOBALS ${this.#constToString(this.constants[this.inst[idx + 1]])}`;
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
                    const tmpl = this.constants[this.inst[idx + 1]] as {params: any[]};
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
        return `NumLocals: ${this.numLocals}\n` + ops.join("\n");
    }

    deepPrint() {
        console.log(this.toString(), "\n")
        for (let i = 0; i < this.constants.length; i++) {
            const c = this.constants[i]
            if (c instanceof ClosureTemplate) {
                console.log(`Const #${i}:\n`)
                c.code.deepPrint()
            }
        }
    }
}

export type UpVarLoc = { index: number, local: boolean }

/** A template for a closure that can then be bound to a scope */
export class ClosureTemplate {
    params: symbol[]; // base (individual param binds)
    remParams: symbol | null; // where the remaining params should be bound too (if any). This implicitly makes a closure variadic as well
    code: ByteCode
    upvarLocs: UpVarLoc[] // what upvars do we need to capture

    constructor(params: symbol[], remParams: symbol | null, code: ByteCode, upvarLocs: UpVarLoc[]) {
        this.params = params
        this.remParams = remParams
        this.code = code
        this.upvarLocs = upvarLocs
    }
}

/** An actual anima closure bound to a scope */
export class Closure extends IProcedure {
    upvars: {value: any}[]
    constructor(public tmpl: ClosureTemplate) {
        super()
        // Allocate enough space for the upvars from outer scopes
        this.upvars = new Array(tmpl.upvarLocs.length)
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

    constructor(nargs: number, bc: ByteCode) {
        super()
        this.nargs = nargs
        this.bc = bc
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

/** The scope the actual anima engine has/uses */
class VmScope {
    public slots: {value: any}[]
    constructor(numLocals: number) {
        const slots = new Array(numLocals)
        for (let i = 0; i < numLocals; i++) {
            slots[i] = {value: undefined}
        }
        this.slots = slots
    }
}

export class Globals {
    constructor(public data: Map<symbol, any>, public frozen: boolean = false) {}

    static newWith(fields: Record<symbol, any>, frozen: boolean = false) {
        const map = new Map()
        Object.getOwnPropertySymbols(fields).forEach((sym) => {
            map.set(sym, fields[sym])
        });
        return new Globals(map, frozen);
    }
}

class CallFrame {
    constructor(
        public code: ByteCode,
        public scope: VmScope,
        public upvars: {value: any}[],
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

    public evaluate(code: ByteCode, scope: Globals, props?: ExposedProps): any {
        return this.#evalinner(code, scope, props);
    }

    #evalinner(code: ByteCode, execScope: Globals, props?: ExposedProps): any {
        // Initial frame and stack
        let frames: CallFrame[] = [new CallFrame(code, new VmScope(code.numLocals), [], 0)];
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
                case OpCode.GETUPVAR: {
                    const slot = frame.readNext()
                    stack.push(frame.upvars[slot].value)

                    break
                }
                case OpCode.GETLOCAL: {
                    const slot = frame.readNext()
                    stack.push(frame.scope.slots[slot].value)
                    break
                }
                case OpCode.GETGLOBALS: {
                    const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                    if (!execScope.data.has(varname)) throw new MissingVarError(`Variable '${String(varname)}' is not defined in the current scope.`)
                    stack.push(execScope.data.get(varname))
                    break
                }
                case OpCode.SETUPVAR: {
                    const slot = frame.readNext()
                    const val = stack.pop()
                    frame.upvars[slot].value = val
                    break
                }
                case OpCode.SETLOCAL: {
                    const slot = frame.readNext()
                    const val = stack.pop()
                    frame.scope.slots[slot].value = val
                    break
                }
                case OpCode.SETGLOBALS: {
                    const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                    const val = stack.pop()
                    if (execScope.frozen) throw new Error(`Variable '${String(varname)}' cannot be set in a frozen scope.`);
                    if (!execScope.data.has(varname)) throw new MissingVarError(`Variable '${String(varname)}' is not defined in the current scope.`)
                    execScope.data.set(varname, val)
                    break
                }
                case OpCode.DEFINEVAR: {
                    const varname = frame.getConst(frame.readNext()) as symbol // compiler ensures its a symbol
                    const val = stack.pop()
                    if (execScope.frozen) throw new Error(`Variable '${String(varname)}' cannot be defined in a frozen scope.`);
                    execScope.data.set(varname, val)
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
                    this.#dispatchCall(target, nargs, isTail, frame, frames, stack);
                    break;
                }
                case OpCode.NEWCLOSURE: {
                    const template = frame.getConst(frame.readNext()) as ClosureTemplate
                    const closure = new Closure(template)
                    // Copy over upvalues
                    for (let i = 0; i < template.upvarLocs.length; i++) {
                        const loc = template.upvarLocs[i]
                        if (loc.local) {
                            closure.upvars[i] = frame.scope.slots[loc.index];
                        } else {
                            // Grab from the current frame's upvars
                            closure.upvars[i] = frame.upvars[loc.index];
                        }
                    }
                    stack.push(closure);
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
                    this.#dispatchCall(target, finalArgs.length, isTail, frame, frames, stack);
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
     */
    #dispatchCall(
        target: any, 
        nargs: number, 
        isTail: boolean, 
        frame: CallFrame, 
        frames: CallFrame[], 
        stack: any[], 
    ) {
        const stackBase = stack.length - nargs; // where does stack start
        if (target instanceof BuiltinFunction) {
            if (target.nargs !== -1 && nargs !== target.nargs) {
                throw new Error(`Builtin expected ${target.nargs} args, got ${nargs}`);
            }
            
            if (target.nargs === -1) stack.push(nargs);

            // BuiltinFunction's reuse existing scope and upvars
            if (isTail) {
                frame.code = target.bc;
                frame.ip = 0; 
            } else {
                frames.push(new CallFrame(target.bc, frame.scope, frame.upvars, 0));
            }
        } else if (target instanceof NativeFunction) {
            if (target.nargs !== -1 && target.nargs !== nargs) {
                throw new Error(`${target.name}: expected ${target.nargs} args, got ${nargs}`);
            }
            
            const args = nargs > 0 ? stack.slice(stackBase) : [];
            stack.length = stackBase;
            
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
            const callScope = new VmScope(template.code.numLocals);
            
            // required
            for (let i = 0; i < arity; i++) {
                callScope.slots[i].value = stack[stackBase + i];
            }
            // variadic
            if (template.remParams !== null) {
                const restArgs = [];
                for (let i = arity; i < nargs; i++) {
                    restArgs.push(stack[stackBase + i]);
                }
                callScope.slots[arity].value = restArgs;
            }

            stack.length = stackBase
            if (isTail) {
                // Reuse existing frame (TCO)
                frame.code = template.code;
                frame.upvars = target.upvars
                frame.scope = callScope
                frame.ip = 0; 
            } else {
                frames.push(new CallFrame(template.code, callScope, target.upvars, 0));
            }
        } else {
            throw new Error(`Attempted to call a non-procedure: ${String(target)}`);
        }
    }
}