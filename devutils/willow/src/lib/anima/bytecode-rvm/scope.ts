import type { UpVarLoc } from "./vm";

export type Resolve = { type: "Global" } | { type: "Local", index: number } | { type: "Upvar", index: number }

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

    get numLocals() {
        return this.regAlloc.total
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
    next: number = 0 // next reg number
    freelist: number[] = []

    alloc(): number {
        if (this.freelist.length > 0) {
            return this.freelist.pop() as number
        } else {
            return this.next++
        }
    }

    free(reg: number) {
        this.freelist.push(reg);
    }

    // allocates a continous block of n registers starting from n
    allocBlock(n: number): number {
        const start = this.next;
        this.next += n;
        return start;
    }

    freeBlock(start: number, n: number) {
        for (let i = 0; i < n; i++) {
            this.free(start + i);
        }
    }

    get total() { return this.next }
}