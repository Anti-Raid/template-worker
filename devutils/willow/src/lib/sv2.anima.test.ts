import { Anima, ASP, ASPParseError, ASPTokenError, MissingVarError } from './sv2.anima'; // Update path as needed
import { describe, it, expect, beforeEach } from 'vitest';

// Made with help of gemini cli
describe('Anima', () => {
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

    const run = (expr: any) => evaluator.evaluate(expr, baseData);

    describe('Primitives & Symbols', () => {
        it('evaluates boolean primitives', () => {
            expect(run(true)).toBe(true);
            expect(run(false)).toBe(false);
        });

        it('evaluates numbers and raw arrays', () => {
            expect(run(42)).toBe(42);
            expect(run([])).toStrictEqual([]);
        });

        it('evaluates implicit variables (Symbols)', () => {
            expect(run("port")).toBe(8080);
            expect(run("protocol")).toBe("tcp");
        });

        it('evaluates literal strings (Prefix)', () => {
            expect(run("'admin")).toBe("admin");
            expect(run("'port")).toBe("port"); // Literal string, not the variable!
        });

        it('errors for unknown variables', () => {
            expect(() => run("missing_var")).toThrow(MissingVarError);
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
            expect(run(["modulo", 10, 3])).toBe(1);
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

        it('Ensure valid TCO', () => {
            const ast = [
                "do",
                ["define", "loop", 
                    ["lambda", ["n"], 
                        ["if", ["==", "n", 0],
                            ["quote", "survived!"],
                            ["loop", ["-", "n", 1]] // Tail position, so no new stack frame
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

    describe('quote operator', () => {
        it('quotes primitive numbers', () => {
            expect(run(["quote", 42])).toBe(42);
        });

        it('quotes strings without treating them as variables', () => {
            // Even if 'x' is defined in the global scope, quote should return the string "x"
            expect(run(["quote", "x"])).toBe("x");
        });

        it('protects lists from being evaluated as function calls', () => {
            // Normally ["+", 1, 2] evaluates to 3. Quote should return the raw array.
            const ast = ["quote", ["+", 1, 2]];
            const result = run(ast);
            expect(result).toEqual(["+", 1, 2]); // Use .toEqual for deep array comparison
        });

        it('protects nested lists perfectly', () => {
            const ast = ["quote", [1, [2, 3], 4]];
            expect(run(ast)).toEqual([1, [2, 3], 4]);
        });

        it('handles nested quotes', () => {
            // Quoting a quote should return the inner quote array
            const ast = ["quote", ["quote", 100]];
            expect(run(ast)).toEqual(["quote", 100]);
        });

        it('evaluates single-quote string sugar correctly', () => {
            expect(run("'hello")).toBe("hello");
            expect(run("'admin")).toBe("admin");
        });

        it('works seamlessly with car and cdr', () => {
            // (car (quote (1 2 3))) -> 1
            expect(run(["car", ["quote", [1, 2, 3]]])).toBe(1);
            
            // (cdr (quote (1 2 3))) -> [2, 3]
            expect(run(["cdr", ["quote", [1, 2, 3]]])).toEqual([2, 3]);
        });

        it('throws an error if given too many arguments', () => {
            expect(() => run(["quote", 1, 2])).toThrow();
        });

        it('throws an error if given no arguments', () => {
            expect(() => run(["quote"])).toThrow();
        });

        it('quine test', () => {
            const ast = [
                ["lambda", ["f", "k"], 
                    ["list", "f", ["list", ["quote", "quote"], "f"], "k"]
                ],
                ["quote", 
                    ["lambda", ["f", "k"], 
                    ["list", "f", ["list", ["quote", "quote"], "f"], "k"]
                    ]
                ],
                304
            ]

            expect(run(ast)).toStrictEqual(ast);
        })
    })
});

describe('Anima String Parser (ASP)', () => {
    describe('Primitives', () => {
        it('parses numbers', () => {
            expect(new ASP("42").parse()).toBe(42);
            expect(new ASP("-3.14").parse()).toBe(-3.14);
        });

        it('parses booleans and null', () => {
            expect(new ASP("true").parse()).toBe(true);
            expect(new ASP("false").parse()).toBe(false);
            expect(new ASP("null").parse()).toBe(null);
        });

        it('parses symbols/variables', () => {
            expect(new ASP("my-var").parse()).toBe("my-var");
            expect(new ASP("+").parse()).toBe("+");
            expect(new ASP("is_admin?").parse()).toBe("is_admin?");
        });
    });

    describe('Literal String', () => {
        it('parses standard strings into quote calls', () => {
            expect(new ASP('"hello"').parse()).toEqual(["quote", "hello"]);
        });

        it('handles escaped quotes and newlines', () => {
            const input = '"She said \\"Hello\\"\\nNext line"';
            const expected = "She said \"Hello\"\nNext line"; 
            expect(new ASP(input).parse()).toEqual(["quote", expected]);
        });
    });

    describe('Lists', () => {
        it('parses standard parentheses', () => {
            expect(new ASP("(+ 1 2)").parse()).toEqual(["+", 1, 2]);
        });

        it('parses square brackets', () => {
            expect(new ASP("[define x 10]").parse()).toEqual(["define", "x", 10]);
        });

        it('handles deeply nested lists', () => {
            expect(new ASP("(if (> age 18) [print \"adult\"] null)").parse()).toEqual([
                "if",
                [">", "age", 18],
                ["print", ["quote", "adult"]],
                null
            ]);
        });

        it('parses empty lists', () => {
            expect(new ASP("()").parse()).toEqual([]);
            expect(new ASP("[]").parse()).toEqual([]);
        });
    });

    describe('Quotes', () => {
        it('quotes symbols', () => {
            expect(new ASP("'a").parse()).toEqual(["quote", "a"]);
        });

        it('quotes lists', () => {
            expect(new ASP("'(1 2 3)").parse()).toEqual(["quote", [1, 2, 3]]);
        });

        it('handles nested quotes correctly', () => {
            expect(new ASP("''a").parse()).toEqual(["quote", ["quote", "a"]]);
        });
        
        it('handles quote right next to parentheses without spaces', () => {
            expect(new ASP("'(\"a\" \"b\")").parse()).toEqual([
                "quote", 
                [["quote", "a"], ["quote", "b"]]
            ]);
        });
    });

    describe('Trivia (Whitespace and Comments)', () => {
        it('ignores leading, trailing, and excessive whitespace', () => {
            expect(new ASP("   \n\t  (+   1   2)  \n ").parse()).toEqual(["+", 1, 2]);
        });

        it('ignores single-line comments completely', () => {
            const script = `
                ; This is a config file
                (define port 8080) ; Set the port
                (start port) ; start it up
            `;
            expect(new ASP(script).parse()).toEqual([
                "do",
                ["define", "port", 8080],
                ["start", "port"]
            ]);
        });
    });

    describe('Multiple Expressions (wrapped in do so last expr is result)', () => {
        it('returns null for completely empty input', () => {
            expect(new ASP("").parse()).toBe(null);
            expect(new ASP("   ; just a comment   ").parse()).toBe(null);
        });

        it('returns raw expression if only one root exists', () => {
            expect(new ASP("(+ 1 2)").parse()).toEqual(["+", 1, 2]);
        });

        it('wraps multiple roots in a "do" block', () => {
            expect(new ASP("1 2 3").parse()).toEqual(["do", 1, 2, 3]);
            expect(new ASP("(def x 1) (def y 2)").parse()).toEqual([
                "do", 
                ["def", "x", 1], 
                ["def", "y", 2]
            ]);
        });
    });

    describe('Error Handling', () => {
        it('throws ASPTokenError for unterminated strings', () => {
            expect(() => new ASP('"this string never ends').parse())
                .toThrow(ASPTokenError);
        });

        it('throws ASPParseError for missing closing brackets', () => {
            expect(() => new ASP("(+ 1 2").parse()).toThrow(ASPParseError);
            expect(() => new ASP("[define x 10").parse()).toThrow(ASPParseError);
        });

        it('throws ASPParseError for unexpected closing brackets', () => {
            expect(() => new ASP("(+ 1 2))").parse()).toThrow(ASPParseError);
            expect(() => new ASP("]").parse()).toThrow(ASPParseError);
            expect(() => new ASP("1 2 ]").parse()).toThrow(ASPParseError);
        });
        
        it('throws ASPParseError for trailing quote without expression', () => {
            // A quote at the very end of the file with nothing to quote
            expect(() => new ASP("'").parse()).toThrow(ASPParseError);
            expect(() => new ASP("(list 1 2 ')").parse()).toThrow(ASPParseError);
        });
    });
});