import type { JumpLabel } from "./ir";
import type { UpVarLoc } from "./vm";

export type Resolve = { type: "Global" } | { type: "Local", index: number } | { type: "Upvar", index: number } | { type: 'Contified', label: JumpLabel, paramRegs: number[] };

export class VariableMetadata {
    get isBoxed() { return this.isCaptured && this.mutable }

    constructor(public mutable: boolean = false, public isCaptured: boolean = false, ) {}
}

export class AnalysisScope {
    #vals = new Map<symbol, VariableMetadata>()
    outer: AnalysisScope | null;

    constructor(outer: AnalysisScope | null) {
        this.outer = outer;
    }

    dbgPrint() {
        for(const [sym, md] of this.#vals.entries()) {
            console.debug(JSON.stringify({sym: sym.description, md}))
        }
    }

    define(sym: symbol) {
        this.#vals.set(sym, new VariableMetadata());
    }

    getVarinfo(sym: symbol): VariableMetadata | null {
        if (this.#vals.has(sym)) return this.#vals.get(sym)!;
        if (this.outer) return this.outer.getVarinfo(sym);
        return null;
    }

    readVar(sym: symbol): boolean {
        if (this.#vals.has(sym)) {
            return true; // Found locally
        }
        
        if (this.outer && this.outer.readVar(sym)) {
            this.outer.markCaptured(sym);
            return true;
        }
        return false;
    }

    markCaptured(sym: symbol) {
        if (this.#vals.has(sym)) {
            this.#vals.get(sym)!.isCaptured = true;
        } else if (this.outer) {
            this.outer.markCaptured(sym);
        }
    }

    markMutable(sym: symbol) {
        if (this.#vals.has(sym)) {
            this.#vals.get(sym)!.mutable = true;
        } else if (this.outer) {
            this.outer.markMutable(sym);
        }
    }
}

/** 
 * Tracks/'simulates' block-level variable shadowing (within a function) at compile-time 
 * 
 * Used internally for optimizing out IIFE's etc.
*/
export class Block {
    #bindings = new Map<symbol, number>();
    #allocatedRegs = new Map<number, symbol>();
    parent: Block | null;

    constructor(parent: Block | null = null) {
        this.parent = parent;
    }

    bind(sym: symbol, reg: number) {
        this.#bindings.set(sym, reg)
        this.#allocatedRegs.set(reg, sym)
    }

    getBoundRegs() {
        return this.#allocatedRegs.keys()
    }

    isRegAllocated(reg: number): boolean {
        if (this.#allocatedRegs.has(reg)) return true
        if (this.parent) return this.parent.isRegAllocated(reg)
        return false
    }

    // Walks up the nested blocks (within the SAME function) to find the slot
    resolve(sym: symbol): number | null {
        if (this.#bindings.has(sym)) return this.#bindings.get(sym)!;
        if (this.parent) return this.parent.resolve(sym);
        return null;
    }
}

/** Helper utility for keeping track of variable scoping */
export class CompilerScope {
    // Keeps track of variables that have been shadowed etc.
    currBlock: Block = new Block();
    regAlloc: RegAlloc = new RegAlloc();

    outer: CompilerScope | null;
    upvars: UpVarLoc[] = [];

    constructor(outer: CompilerScope | null) {
        this.outer = outer;
        this.upvars = []
    }

    get numRegs() {
        return this.regAlloc.total
    }

    enterBlock() {
        this.currBlock = new Block(this.currBlock);
    }

    exitBlock() {
        for (const reg of this.currBlock.getBoundRegs()) {
            this.regAlloc.free(reg);
        }

        if (this.currBlock.parent) {
            this.currBlock = this.currBlock.parent;
        } else {
            throw new Error("internal error: cannot exit root block of CompilerScope.");
        }
    }
    
    // Returns the register the variable will be at
    addLocal(sym: symbol) {
        const reg = this.regAlloc.alloc();
        this.currBlock.bind(sym, reg);
        return reg;
    }

    allocTemp(): number {
        return this.regAlloc.alloc();
    }

    freeTemp(reg: number) {
        // If its not allocated on the block, then we can free it, otherwise, we cant
        if (!this.currBlock.isRegAllocated(reg)) {
            this.regAlloc.free(reg);
        }
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

export class RegAlloc {
    used: Uint8Array = new Uint8Array(64); 
    nreg: number = 0;

    alloc(): number {
        // Look for a reg we can reuse
        for (let i = 0; i < this.nreg; i++) {
            if (this.used[i] === 0) {
                this.used[i] = 1;
                return i;
            }
        }
        // Worst case: expand the nregs
        return this.#expandAndClaim(1);
    }

    allocBlock(n: number): number {
        let consecutive = 0;
        let start = -1;
        
        // Look for a consecutive block of reg's we can reuse
        for (let i = 0; i < this.nreg; i++) {
            if (this.used[i] === 0) {
                if (consecutive === 0) start = i;
                consecutive++;
                
                if (consecutive === n) {
                    this.used.fill(1, start, start + n);
                    return start;
                }
            } else {
                consecutive = 0;
            }
        }
        // Worst case: expand the nregs
        return this.#expandAndClaim(n);
    }

    free(reg: number) {
        this.used[reg] = 0;
    }

    freeBlock(start: number, n: number) {
        this.used.fill(0, start, start + n);
    }

    #expandAndClaim(n: number): number {
        const start = this.nreg;
        this.nreg += n;
    
        // Resize the used array
        if (this.nreg > this.used.length) {
            const newArray = new Uint8Array(this.used.length * 2);
            newArray.set(this.used);
            this.used = newArray;
        }
        
        this.used.fill(1, start, start + n);
        return start;
    }

    get total() { return this.nreg; }
}