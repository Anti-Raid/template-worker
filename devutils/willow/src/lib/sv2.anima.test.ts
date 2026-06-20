import { Anima, ASP, ASPParseError, ASPTokenError, MissingVarError } from './sv2.anima'; // Update path as needed
import { describe, it, expect, beforeEach } from 'vitest';

// Helper for brevity when writing manual ASTs
const s = Symbol.for;

describe('Anima', () => {
    let evaluator: Anima;
    let baseData: Record<string, any>;

    beforeEach(() => {
        evaluator = new Anima();
        baseData = {
            port: 8080,
            protocol: "tcp",
            is_active: true,
            user_role: null 
        };
    });

    const run = (expr: any) => evaluator.evaluate(expr, baseData);

    describe('Primitives, Strings & Symbols', () => {
        it('evaluates boolean primitives', () => {
            expect(run(true)).toBe(true);
            expect(run(false)).toBe(false);
        });

        it('evaluates numbers and raw arrays', () => {
            expect(run(42)).toBe(42);
            expect(run([])).toStrictEqual([]);
        });

        it('evaluates native Symbols as implicit variables', () => {
            expect(run(s("port"))).toBe(8080);
            expect(run(s("protocol"))).toBe("tcp");
        });

        it('evaluates literal JS strings as Scheme strings directly', () => {
            expect(run("hello")).toBe("hello");
            expect(run("port")).toBe("port"); // String primitive, not a variable lookup
        });

        it('errors for unknown variables', () => {
            expect(() => run(s("missing_var"))).toThrow(MissingVarError);
        });
    });

    describe('Logic & Control Flow', () => {
        it('evaluates strict equality', () => {
            expect(run([s("eqv?"), s("port"), 8080])).toBe(true);
            expect(run([s("not"), [s("eqv?"), s("protocol"), "udp"]])).toBe(true);
        });

        it('evaluates if statements using strict truthiness', () => {
            expect(run([s("if"), s("is_active"), "yes", "no"])).toBe("yes");
            expect(run([s("if"), s("user_role"), "yes", "no"])).toBe("no");
            expect(run([s("if"), 0, "yes", "no"])).toBe("yes");
        });

        it('short-circuits AND statements', () => {
            expect(run([s("and"), true, false, s("does_not_exist")])).toBe(false);
        });

        it('short-circuits OR statements and returns actual truthy values', () => {
            expect(run([s("or"), false, s("port"), ["crash!"]])).toBe(8080);
        });
    });

    describe('Math Operations', () => {
        it('performs basic arithmetic', () => {
            expect(run([s("+"), 10, 5])).toBe(15);
            expect(run([s("-"), 10, 5])).toBe(5);
            expect(run([s("*"), 10, 5])).toBe(50);
            expect(run([s("/"), 10, 5])).toBe(2);
            expect(run([s("modulo"), 10, 3])).toBe(1);
        });

        it('performs numeric comparisons', () => {
            expect(run([s(">"), s("port"), 1024])).toBe(true);
            expect(run([s("<"), s("port"), 10000])).toBe(true);
            expect(run([s(">="), 10, 10])).toBe(true);
            expect(run([s("<="), 5, 10])).toBe(true);
        });
    });

    describe('Data Structures & Types', () => {
        it('creates lists and evaluates length', () => {
            expect(run([s("list"), 1, 2, 3])).toEqual([1, 2, 3]);
            expect(run([s("length"), [s("list"), "a", "b"]])).toBe(2);
            expect(run([s("length"), "string_len"])).toBe(10);
        });

        it('checks contains', () => {
            expect(run([s("contains"), [s("list"), 1, 2], 2])).toBe(true);
            expect(run([s("contains"), [s("list"), 1, 2], 3])).toBe(false);
        });

        it('evaluates type? with JS Symbols', () => {
            expect(run([s("type?"), s("port")])).toBe("number");
            expect(run([s("type?"), s("protocol")])).toBe("string");
            expect(run([s("type?"), s("user_role")])).toBe("null");
            
            // Verifying our new native Symbol logic!
            expect(run([s("type?"), [s("quote"), s("my_symbol")]])).toBe("symbol");
        });
    });

    describe('Lexical Scoping & Closures', () => {
        it('executes DO sequences and DEFINEs variables', () => {
            const ast = [
                s("do"),
                [s("define"), s("x"), 10],
                [s("define"), s("y"), 20],
                [s("+"), s("x"), s("y")]
            ];
            expect(run(ast)).toBe(30);
            expect(baseData.x).toBeUndefined(); 
        });

        it('creates and calls a lambda with arguments', () => {
            const ast = [
                s("do"),
                [s("define"), s("add"), 
                    [s("lambda"), [s("a"), s("b")], [s("+"), s("a"), s("b")]]
                ],
                [s("add"), 5, 7]
            ];
            expect(run(ast)).toBe(12);
        });

        it('creates and calls a lambda with multiple body expressions (implicit do)', () => {
            const ast = [
                s("do"),
                [s("define"), s("counter"), 0],
                [s("define"), s("increment"), 
                    [s("lambda"), [], 
                        [s("define"), s("counter"), [s("+"), s("counter"), 1]], // Body Expr 1
                        s("counter")                                            // Body Expr 2
                    ]
                ],
                [s("increment")]
            ];
            expect(run(ast)).toBe(1);
        });

        it('respects closure scope (variables enclosed at creation)', () => {
            const ast = [
                s("do"),
                [s("define"), s("x"), 100], 
                [s("define"), s("make_adder"), 
                    [s("lambda"), [s("y")], [s("+"), s("x"), s("y")]] 
                ],
                [s("define"), s("x"), 999], 
                [s("make_adder"), 5] 
            ];
            expect(run(ast)).toBe(1004);
        });

        it('Ensure valid TCO', () => {
            const ast = [
                s("do"),
                [s("define"), s("loop"), 
                    [s("lambda"), [s("n")], 
                        [s("if"), [s("="), s("n"), 0],
                            "survived!",
                            [s("loop"), [s("-"), s("n"), 1]]
                        ]
                    ]
                ],
                [s("loop"), 15000]
            ];
            expect(() => run(ast)).not.toThrow();
            expect(run(ast)).toBe("survived!");
        });
    });

    describe('quote operator & Native Symbols', () => {
        it('quotes primitive numbers', () => {
            expect(run([s("quote"), 42])).toBe(42);
        });

        it('quotes Symbols perfectly', () => {
            expect(run([s("quote"), s("x")])).toBe(s("x"));
        });

        it('protects lists and retains inner native types', () => {
            const ast = [s("quote"), [s("+"), 1, 2]];
            expect(run(ast)).toEqual([s("+"), 1, 2]);
        });

        it('protects nested lists perfectly', () => {
            const ast = [s("quote"), [1, [2, 3], 4]];
            expect(run(ast)).toEqual([1, [2, 3], 4]);
        });

        it('handles nested quotes recursively', () => {
            const ast = [s("quote"), [s("quote"), 100]];
            expect(run(ast)).toEqual([s("quote"), 100]);
        });

        it('throws an error if given too many arguments', () => {
            expect(() => run([s("quote"), 1, 2])).toThrow();
        });
    });

    describe('cond special form', () => {
        it('Matches the first truthy condition', () => {
            const ast = [
                s("cond"),
                [true, "first"],
                [true, "second"]
            ];
            expect(run(ast)).toBe("first");
        });

        it('Skips falsey conditions and matches later ones', () => {
            const ast = [
                s("cond"),
                [false, "first"],
                [[s("="), 1, 2], "second"],
                [[s(">"), 5, 3], "third"]
            ];
            expect(run(ast)).toBe("third");
        });

        it('Falls back to the else clause if nothing matches', () => {
            const ast = [
                s("cond"),
                [false, "first"],
                [null, "second"],
                [s("else"), "fallback"]
            ];
            expect(run(ast)).toBe("fallback");
        });

        it('Returns null if no conditions match and there is no else clause', () => {
            const ast = [
                s("cond"),
                [false, "first"],
                [[s("="), 1, 2], "second"]
            ];
            expect(run(ast)).toBeNull();
        });
    });
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

        it('parses symbols natively', () => {
            expect(new ASP("my-var").parse()).toBe(s("my-var"));
            expect(new ASP("+").parse()).toBe(s("+"));
        });
    });

    describe('Literal String', () => {
        it('parses standard strings into raw JS strings', () => {
            expect(new ASP('"hello"').parse()).toBe("hello");
        });

        it('handles escaped quotes and newlines', () => {
            const input = '"She said \\"Hello\\"\\nNext line"';
            const expected = "She said \"Hello\"\nNext line"; 
            expect(new ASP(input).parse()).toBe(expected);
        });
    });

    describe('Lists', () => {
        it('parses standard parentheses', () => {
            expect(new ASP("(+ 1 2)").parse()).toEqual([s("+"), 1, 2]);
        });

        it('parses square brackets', () => {
            expect(new ASP("[define x 10]").parse()).toEqual([s("define"), s("x"), 10]);
        });

        it('handles deeply nested lists', () => {
            expect(new ASP("(if (> age 18) [print \"adult\"] null)").parse()).toEqual([
                s("if"),
                [s(">"), s("age"), 18],
                [s("print"), "adult"],
                null
            ]);
        });
    });

    describe('Quotes', () => {
        it('quotes symbols', () => {
            expect(new ASP("'a").parse()).toEqual([s("quote"), s("a")]);
        });

        it('quotes lists', () => {
            expect(new ASP("'(1 2 3)").parse()).toEqual([s("quote"), [1, 2, 3]]);
        });

        it('handles quote right next to parentheses without spaces', () => {
            expect(new ASP("'(\"a\" \"b\")").parse()).toEqual([
                s("quote"), 
                ["a", "b"]
            ]);
        });
    });

    describe('Trivia (Whitespace and Comments)', () => {
        it('ignores single-line comments completely', () => {
            const script = `
                ; This is a config file
                (define port 8080) ; Set the port
                (start port) ; start it up
            `;
            expect(new ASP(script).parse()).toEqual([
                s("do"),
                [s("define"), s("port"), 8080],
                [s("start"), s("port")]
            ]);
        });
    });

    describe('Multiple Expressions (wrapped in do)', () => {
        it('wraps multiple roots in a "do" block', () => {
            expect(new ASP("1 2 3").parse()).toEqual([s("do"), 1, 2, 3]);
        });
    });

    describe('Error Handling', () => {
        it('throws ASPTokenError for unterminated strings', () => {
            expect(() => new ASP('"this string never ends').parse())
                .toThrow(ASPTokenError);
        });

        it('throws ASPParseError for trailing quote without expression', () => {
            expect(() => new ASP("'").parse()).toThrow(ASPParseError);
        });
    });
});