// Made w/ lots of help from gemini cli
import { Anima, ASP, ASPParseError, ASPTokenError, ASTStringifier, MissingVarError } from './sv2.anima'; 
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
            expect(run([])).toBe(null);
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
            expect(run([s("type?"), s("user_role")])).toBe("list");
            
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

    describe('Cons, Arrays & FFI Boundary', () => {
        it('constructs a proper list and extracts values (car/cdr)', () => {
            expect(run([s("car"), [s("cons"), 1, [s("cons"), 2, null]]])).toBe(1);
            expect(run([s("car"), [s("cdr"), [s("cons"), 1, [s("cons"), 2, null]]]])).toBe(2);
        });

        it('supports improper lists perfectly', () => {
            // (cons 1 2) -> an improper pair
            const ast = [s("cons"), 1, 2];
            
            expect(run([s("car"), ast])).toBe(1);
            expect(run([s("cdr"), ast])).toBe(2);
            
            // length of an improper list with 1 cons pair is 1
            expect(run([s("length"), ast])).toBe(1);
        });

        it('triggers the O(1) Fast Path for raw JS arrays', () => {
            // We use `quote` to generate a raw JS array in the AST
            const rawArray = [s("quote"), [10, 20, 30]];
            
            // car natively peeks at [0]
            expect(run([s("car"), rawArray])).toBe(10);
            
            // cdr natively wraps the array in a Cons view, letting us extract [1]
            expect(run([s("car"), [s("cdr"), rawArray]])).toBe(20);
            
            // length reads the native .length property
            expect(run([s("length"), rawArray])).toBe(3);
        });

        it('handles array-to-cons gatekeeping in cons', () => {
            const ast = [s("cons"), 1, [s("quote"), [2, 3]]];
            
            expect(run([s("car"), ast])).toBe(1);
            expect(run([s("car"), [s("cdr"), ast]])).toBe(2);
            expect(run([s("length"), ast])).toBe(3); // 1 + the array length of 2
        });

        it('throws errors for empty lists', () => {
            // car of null
            expect(() => run([s("car"), null])).toThrow();
            
            // car of empty array
            expect(() => run([s("car"), [s("quote"), []]])).toThrow();
            
            // cdr of empty array
            expect(() => run([s("cdr"), [s("quote"), []]])).toThrow();
        });

        it('traverses hybrid structures with standard operators (last, contains)', () => {
            const pureCons = [s("cons"), "a", [s("cons"), "b", null]];
            const rawArray = [s("quote"), ["a", "b"]];

            // OP_CONTAINS
            expect(run([s("contains"), pureCons, "b"])).toBe(true);
            expect(run([s("contains"), pureCons, "c"])).toBe(false);
            expect(run([s("contains"), rawArray, "b"])).toBe(true);

            // OP_LAST
            expect(run([s("last"), pureCons])).toBe("b");
            expect(run([s("last"), rawArray])).toBe("b");
            
            const improper = [s("cons"), "a", "b"];
            expect(run([s("last"), improper])).toBe("b");
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
            expect(new ASP("#t").parse()).toBe(true);
            expect(new ASP("#f").parse()).toBe(false);
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

describe('ASTStringifier', () => {
    const stringifier = new ASTStringifier();

    const roundtrip = (input: string) => {
        const ast = new ASP(input).parse();
        return stringifier.stringify(ast);
    };

    describe('Primitives & Strings', () => {
        it('round-trips numbers, booleans, and null', () => {
            expect(roundtrip("42")).toBe("42");
            expect(roundtrip("-3.14")).toBe("-3.14");
            expect(roundtrip("true")).toBe("true");
            expect(roundtrip("false")).toBe("false");
            expect(roundtrip("null")).toBe("null");
        });

        it('round-trips symbols perfectly', () => {
            expect(roundtrip("my-variable")).toBe("my-variable");
            expect(roundtrip("+")).toBe("+");
        });

        it('round-trips strings (preserving the literal quotes)', () => {
            expect(roundtrip('"hello"')).toBe('"hello"');
            expect(roundtrip('"string with spaces"')).toBe('"string with spaces"');
        });
    });

    describe('Lists & Trivia Normalization', () => {
        it('round-trips standard lists', () => {
            expect(roundtrip("(+ 1 2)")).toBe("(+ 1 2)");
            expect(roundtrip("(define x 10)")).toBe("(define x 10)");
        });

        it('normalizes square brackets into standard parens', () => {
            expect(roundtrip("[+ 1 2]")).toBe("(+ 1 2)");
            expect(roundtrip("(if [> x 5] true false)")).toBe("(if (> x 5) true false)");
        });

        it('normalizes excess whitespace', () => {
            const sloppyInput = "(  +    1      2   )";
            expect(roundtrip(sloppyInput)).toBe("(+ 1 2)");
        });

        it('completely strips out comments', () => {
            const inputWithComments = `
                (define port 8080) ; this is the port
            `;
            expect(roundtrip(inputWithComments)).toBe("(define port 8080)");
        });
    });

    describe('Syntactic Sugar & Implicit Wrappers', () => {
        it('expands quotes into standard list form', () => {
            expect(roundtrip("'x")).toBe("(quote x)");
            expect(roundtrip("'(1 2 3)")).toBe("(quote (1 2 3))");
        });

        it('exposes implicit "do" for multiple expressions', () => {
            const multiExpr = "(define x 1) (+ x 2)";
            expect(roundtrip(multiExpr)).toBe("(do (define x 1) (+ x 2))");
        });
    });

    describe('Deep Nesting', () => {
        it('complex logic', () => {
            const complexScript = "(define fib (lambda (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))))";
            expect(roundtrip(complexScript)).toBe(complexScript);
        });
    });
});

describe('Complex Tests', () => {
    let evaluator: Anima;
    let baseData: Record<string, any>;

    beforeEach(() => {
        evaluator = new Anima();
        baseData = {};
    });

    const stringifier = new ASTStringifier();
    const run = (expr: string) => {
        const ast = new ASP(expr).parse();
        return stringifier.stringify(evaluator.evaluate(ast, baseData));
    };

    describe('Complex tests', () => {
        it('my-set?', () => {
            expect(run(`
(define my-set?
  (lambda (a)
    (define (in a rst) 
      (cond 
         [(empty? rst) #f]
         [(equal? a (car rst)) #t]
         [else (in a (cdr rst))]))

    (cond 
      [(empty? a) #t]
      [else 
        (if (in (car a) (cdr a)) #f (my-set? (cdr a)))])))

(my-set? (list 1 2 3 4 5))`
)).toBe("#t");

            expect(run(`
(define my-set?
  (lambda (a)
    (define (in a rst) 
      (cond 
         [(empty? rst) #f]
         [(equal? a (car rst)) #t]
         [else (in a (cdr rst))]))

    (cond 
      [(empty? a) #t]
      [else 
        (if (in (car a) (cdr a)) #f (my-set? (cdr a)))])))

(my-set? (list 1 2 3 4 4))`
)).toBe("#f");
        });
    });
});
