import { Anima } from './sv2.brancheval'; // Update path as needed
import { describe, it, expect, beforeEach } from 'vitest';

// Made with help of gemini cli
describe('FormBranchEvaluator', () => {
    let evaluator: Anima;
    let baseData: Record<string, any>;

    beforeEach(() => {
        evaluator = new Anima();
        baseData = {
            port: 8080,
            protocol: "tcp",
            is_active: true,
            user_role: null // Testing the falsy null logic!
        };
    });

    const run = (expr: any) => evaluator.computeBranch(expr, baseData);

    describe('Primitives & Symbols', () => {
        it('evaluates boolean primitives', () => {
            expect(run(true)).toBe(true);
            expect(run(false)).toBe(false);
        });

        it('evaluates numbers and raw arrays', () => {
            expect(run(42)).toBe(42);
            expect(run([])).toBe(null);
        });

        it('evaluates implicit variables (Symbols)', () => {
            expect(run("port")).toBe(8080);
            expect(run("protocol")).toBe("tcp");
        });

        it('evaluates literal strings (Prefix)', () => {
            expect(run("'admin")).toBe("admin");
            expect(run("'port")).toBe("port"); // Literal string, not the variable!
        });

        it('returns null for unknown variables', () => {
            expect(run("missing_var")).toBe(null);
        });
    });

    describe('Logic & Control Flow', () => {
        it('evaluates strict equality', () => {
            expect(run(["==", "port", 8080])).toBe(true);
            expect(run(["!=", "protocol", "'udp"])).toBe(true);
        });

        it('evaluates if statements using strict truthiness', () => {
            // true -> true branch
            expect(run(["if", "is_active", "'yes", "'no"])).toBe("yes");
            
            // null -> false branch
            expect(run(["if", "user_role", "'yes", "'no"])).toBe("no");
            
            // 0 -> true branch (Scheme semantics!)
            expect(run(["if", 0, "'yes", "'no"])).toBe("yes");
        });

        it('short-circuits AND statements', () => {
            // Should stop at #f and never evaluate the missing variable
            expect(run(["and", true, false, "does_not_exist"])).toBe(false);
        });

        it('short-circuits OR statements and returns actual truthy values', () => {
            // Should stop at port and return 8080, bypassing the crash!
            expect(run(["or", false, "port", ["crash!"]])).toBe(8080);
        });
    });

    describe('Math Operations', () => {
        it('performs basic arithmetic', () => {
            expect(run(["+", 10, 5])).toBe(15);
            expect(run(["-", 10, 5])).toBe(5);
            expect(run(["*", 10, 5])).toBe(50);
            expect(run(["/", 10, 5])).toBe(2);
            expect(run(["%", 10, 3])).toBe(1);
        });

        it('performs numeric comparisons', () => {
            expect(run([">", "port", 1024])).toBe(true);
            expect(run(["<", "port", 10000])).toBe(true);
            expect(run([">=", 10, 10])).toBe(true);
            expect(run(["<=", 5, 10])).toBe(true);
        });
    });

    describe('Data Structures & Types', () => {
        it('creates lists and evaluates length', () => {
            expect(run(["list", 1, 2, 3])).toEqual([1, 2, 3]);
            expect(run(["length", ["list", "'a", "'b"]])).toBe(2);
            expect(run(["length", "'string_len"])).toBe(10);
        });

        it('checks contains', () => {
            expect(run(["contains", ["list", "'admin", "'mod"], "'mod"])).toBe(true);
            expect(run(["contains", ["list", "'admin", "'mod"], "'user"])).toBe(false);
        });

        it('evaluates type?', () => {
            expect(run(["type?", "port"])).toBe("number");
            expect(run(["type?", "protocol"])).toBe("string");
            expect(run(["type?", "is_active"])).toBe("boolean");
            expect(run(["type?", "user_role"])).toBe("null");
        });
    });

    describe('Lexical Scoping & Closures', () => {
        it('executes DO sequences and DEFINEs variables', () => {
            const ast = [
                "do",
                ["define", "x", 10],
                ["define", "y", 20],
                ["+", "x", "y"]
            ];
            expect(run(ast)).toBe(30);
            // Ensure global scope wasn't permanently mutated by computeBranch wrapper
            expect(baseData.x).toBeUndefined(); 
        });

        it('creates and calls a lambda with arguments', () => {
            const ast = [
                "do",
                ["define", "add", 
                    ["lambda", ["a", "b"], ["+", "a", "b"]]
                ],
                ["add", 5, 7]
            ];
            expect(run(ast)).toBe(12);
        });

        it('respects closure scope (variables enclosed at creation)', () => {
            const ast = [
                "do",
                ["define", "x", 100], // Outer x
                ["define", "make_adder", 
                    ["lambda", ["y"], ["+", "x", "y"]] // Captures outer x
                ],
                ["define", "x", 999], // This shouldn't affect the created lambda's lexical scope if implemented strictly, but in standard mutable environments it might. Let's test standard behavior!
                ["make_adder", 5] 
            ];
            // Note: Since `define` overwrites the current scope dictionary, 
            // the closure will look up 'x' and find 999. This is correct JS/Scheme behavior!
            expect(run(ast)).toBe(1004);
        });

        it('STRESS TEST: Tail Call Optimization prevents Stack Overflow', () => {
            // A simple tail-recursive loop that runs 15,000 times.
            // In standard JS recursion, this throws "Maximum call stack size exceeded" around 10k.
            const ast = [
                "do",
                ["define", "loop", 
                    ["lambda", ["n"], 
                        ["if", ["==", "n", 0],
                            "'survived!",
                            ["loop", ["-", "n", 1]] // Tail position!
                        ]
                    ]
                ],
                ["loop", 15000]
            ];
            
            // If TCO fails, this line will throw an error and crash the test.
            expect(() => run(ast)).not.toThrow();
            expect(run(ast)).toBe("survived!");
        });
    });

        it('simple eval', () => {
            const ast = [ 
                [ ["lambda", ["x"], ["lambda", ["y"], ["+", "x", "y"]]], 10 ], 
                5 
            ];
            expect(run(ast)).toBe(15);
        });
});