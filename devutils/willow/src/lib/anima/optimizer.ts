import {
  OP_IF,
  OP_QUOTE,
  OP_EQ,
  OP_ADD,
  OP_SUB,
  OP_MUL,
  OP_DIV,
  OP_MODULO,
  DottedPair,
  OP_REMAINDER
} from "./common";

const FOLDABLE_MATH_OPS = new Set([
    OP_ADD,
    OP_SUB,
    OP_MUL,
    OP_DIV,
    OP_MODULO,
    OP_REMAINDER,
    OP_EQ,
])

// Optimizes a fully transformed AST
export class AnimaOptimizer {
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
