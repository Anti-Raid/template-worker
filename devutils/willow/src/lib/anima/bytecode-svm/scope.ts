import type { UpVarLoc } from "./vm";

export type Resolve = { type: "Global" } | { type: "Local", index: number } | { type: "Upvar", index: number }

/** 
 * Tracks/'simulates' block-level variable shadowing (within a function) at compile-time 
 * 
 * Used internally for optimizing out IIFE's etc.
*/
export class Block {
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
export class CompilerScope {
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