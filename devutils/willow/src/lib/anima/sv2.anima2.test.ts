// Made w/ lots of help from gemini cli
import { MissingVarError, ASP, isDeepEqual } from './common';
import { describe, it, expect } from 'vitest';
import { Cons } from './list';
import { Anima, ByteCode, ASTStringifier } from './bytecode-rvm/anima';
import { deepPrint } from './bytecode-rvm/utils';

const bcCache: Record<string, ByteCode> = {}
describe('Anima', () => {
    let evaluator: Anima = new Anima();
    let s = new ASTStringifier()
    evaluator.scope.set(Symbol.for("port"), 8080)
    evaluator.scope.set(Symbol.for("protocol"), "tcp")
    evaluator.scope.set(Symbol.for("is_active"), true)
    evaluator.scope.set(Symbol.for("user_role"), null)
    
    const run = (expr: string) => {
        if (bcCache[expr]) return s.stringify(evaluator.evaluateRaw(bcCache[expr]))
        const bc = evaluator.compiler.compileRaw(expr)
        deepPrint(bc)
        bcCache[expr] = bc
        return s.stringify(evaluator.evaluateRaw(bc));
    };

    describe('Primitives, Strings & Symbols', () => {
        it('evaluates boolean primitives', () => {
            expect(run("#t")).toBe("#t");
            expect(run("#f")).toBe("#f");
        });

        it('evaluates numbers and raw arrays', () => {
            expect(run("42")).toBe("42");
            expect(run("[]")).toBe("()"); // ASP parses [] to a PUSHEMPTYLIST, VM evals [] as null
        });

        it('evaluates native Symbols as implicit variables', () => {
            expect(run("port")).toBe("8080");
            expect(run("protocol")).toBe("\"tcp\"");
        });

        it('evaluates literal JS strings as Scheme strings directly', () => {
            expect(run('"hello"')).toBe("\"hello\"");
            expect(run('"port"')).toBe("\"port\""); // String primitive, not a variable lookup
        });

        it('errors for unknown variables', () => {
            expect(() => run("missing_var")).toThrow(MissingVarError);
        });

        it('basic math', () => {
            expect(run("(+ (* 1 2) (- 1 1) (- 1 2))")).toBe("1");
            expect(run("(+ (* 1 2) (- 1 1) (- 1 51))")).toBe("-48");
            expect(run("(+ (let [(x 1)] x) 1)")).toBe("2");
        });

        it('Ensure valid TCO', () => {
            const script = `
                (begin
                  (define (loop n)
                    (if (= n 0)
                        "survived!"
                        (loop (- n 1))))
                  (loop 15000))
            `;
            expect(() => run(script)).not.toThrow();
            expect(run(script)).toBe("\"survived!\"");
        });

        it('Ensure valid TCO [2]', () => {
            const script = `
                (begin
                  (define (loop n)
                    (if (= n 0)
                        "survived!"
                        (loop (- n 1))))
                  (loop 15000))
            `;
            expect(() => run(script)).not.toThrow();
            expect(run(script)).toBe("\"survived!\"");
        });

        it('test recursion', () => {
            const script = `
(define (f n)
  (if (= n 0)
      (lambda () n) 
      (f (- n 1))))

((f 10))`

            expect(run(script)).toBe("0")
        })

        it('test recursion and closure capture', () => {
            const script = `
(define (f n)
  (if (= n 0)
      (lambda () n)
      (begin
        (let ((inner-closure (f (- n 1))))
           inner-closure))))

((f 1))
            `
            expect(run(script)).toBe("0")
        })

        it('test upvars/upvalues', () => {
            const script = `
(define (f n)
  (define m n) ; m will now become a upvalue
  (define (o)
    (define (q) (+ 1 m))
    (+ 1 (q))
  )
  (o)
)

(f 1)
            `
            expect(run(script)).toBe("3")

            expect(run(`
(define (test-deep-reach x)
  (define (level1)
    (define (level2)
      (define (level3)
        (+ x 10))  ; level3 uses 'x'
      (level3))    ; level2 just passes through
    (level2))      ; level1 just passes through
  (level1))

(test-deep-reach 5)    
            `)).toBe("15")

            expect(run(`
(define (test-shadowing)
  (let ((x 100))
    (let ((x 20))
      (let ((f (lambda () x)))
        (let ((x 50))
          (f)))))) ; f should still return 20, not 50 or 100

(test-shadowing)
            `)).toBe("20")

            expect(run(`
(define (make-account initial)
  (let ((balance initial))
    (define (withdraw amount)
      (set! balance (- balance amount))
      balance)
    (define (deposit amount)
      (set! balance (+ balance amount))
      balance)
    (withdraw 10)
    (deposit 50)))

(make-account 100)
            `)).toBe("140")

            expect(run(`
(define (make-counter)
  (let ((count 0))
    (lambda ()
      (set! count (+ count 1))
      count)))

(let ((counter-a (make-counter))
      (counter-b (make-counter)))
  (counter-a) ; 1
  (counter-a) ; 2
  (counter-b) ; 1 (Should be completely independent)
  (counter-a))
`)).toBe("3")

            expect(run(`
(define (multiplier factor)
  (lambda (n)
    (* n factor)))

(let ((times-two (multiplier 2))
      (times-five (multiplier 5)))
  (+ (times-two 10) (times-five 10)))
                `)).toBe("70")
        })
    });

    describe('Logic & Control Flow', () => {
        it('evaluates strict equality', () => {
            expect(run("(eqv? port 8080)")).toBe("#t");
            expect(run(`(not (eqv? protocol "udp"))`)).toBe("#t");
        });

        it('evaluates if statements using strict truthiness', () => {
            expect(run(`(if is_active "yes" "no")`)).toEqual('"yes"');
            expect(run(`(if (not (empty? user_role)) "yes" "no")`)).toBe('"no"');
            expect(run(`(if 0 "yes" "no")`)).toEqual('"yes"');
        });

        it('short-circuits AND statements', () => {
            expect(run("(and #t #f does_not_exist)")).toBe("#f");
        });

        it('short-circuits OR statements and returns actual truthy values', () => {
            expect(run("(or #f port (crash!))")).toEqual("8080");
        });
    });

    describe('map', () => {
        it('maps a procedure over a single list', () => {
            const script = `
                (begin
                  (define (double x) (* x 2))
                  (map double '(1 2 3 4)))
            `;
            expect(run(script)).toEqual("(2 4 6 8)");
        });

        it('maps a procedure over a single list [2]', () => {
            const script = `
                (begin
                  (define (double x) (* x 2))
                  (map double '(1 2 3 4)))
            `;
            expect(run(script)).toEqual("(2 4 6 8)");
        });

        it('maps a procedure over multiple lists in parallel', () => {
            const script = `(map + '(1 2 3) '(10 20 30))`;
            expect(run(script)).toEqual("(11 22 33)");
            
            const script3 = `(map + '(1 1 1) '(2 2 2) '(3 3 3))`;
            expect(run(script3)).toEqual("(6 6 6)");
        });

        it('safely terminates when the shortest list is exhausted', () => {
            const script = `(map + '(1 2 3 4 5) '(10 20))`;
            expect(run(script)).toEqual("(11 22)");
        });

        it('errors with prelude in mapped lambda', () => {
            const script = `(map (lambda (x) (%ArrayNew)) '(1 2 3 4 5))`;
            expect(() => run(script)).toThrow(MissingVarError);
        });
    })
})

describe("isDeepEqual: Improper Lists (Dotted Pairs)", () => {
    
    it("should correctly equate identical improper lists", () => {
        // (1 2 . 3)
        const a = Cons.pair(1, Cons.pair(2, 3));
        const b = Cons.pair(1, Cons.pair(2, 3));
        expect(isDeepEqual(a, b)).toBe(true);
    });

    it("should fail when comparing a proper list to an improper list", () => {
        // (1 2 3) 
        const proper = Cons.pair(1, Cons.pair(2, Cons.pair(3, null)));
        // (1 2 . 3)
        const improper = Cons.pair(1, Cons.pair(2, 3));
        
        expect(isDeepEqual(proper, improper)).toBe(false);
    });

    it("should fail when comparing an improper list to a native JS Array", () => {
        // (1 2 3)
        const arr = [1, 2, 3];
        // (1 2 3) as cons
        const arrCons = Cons.pair(1, Cons.pair(2, Cons.pair(3, null)))
        // (1 2 . 3) 
        const improper = Cons.pair(1, Cons.pair(2, 3));
        
        expect(isDeepEqual(arr, arrCons)).toBe(true);
        expect(isDeepEqual(arr, improper)).toBe(false);
    });

    it("should fail when improper lists have different tails", () => {
        // (1 2 . 3)
        const a = Cons.pair(1, Cons.pair(2, 3));
        // (1 2 . 4)
        const b = Cons.pair(1, Cons.pair(2, 4));
        
        expect(isDeepEqual(a, b)).toBe(false);
    });

    it("should correctly handle nested improper lists", () => {
        // (1 (2 . 3) . 4)
        const a = Cons.pair(1, Cons.pair(Cons.pair(2, 3), 4));
        const b = Cons.pair(1, Cons.pair(Cons.pair(2, 3), 4));
        // (1 (2 . 99) . 4)
        const c = Cons.pair(1, Cons.pair(Cons.pair(2, 99), 4));

        expect(isDeepEqual(a, b)).toBe(true);
        expect(isDeepEqual(a, c)).toBe(false);
    });
    
    it("should correctly handle list primitive equalities", () => {
        const a = Cons.pair(1, 2); // (1 . 2)
        const b = 2; // primitive (2)
        expect(isDeepEqual(a, b)).toBe(false);
    });
});

/*
const TEST_PROG = `
(define union
    (lambda (a b)
        (define (in a rst) 
        (cond 
            [(empty? rst) #f]
            [(equal? a (car rst)) #t]
            [else (in a (cdr rst))]))

        (cond
        ; if either set is empty, the other one if the union
        [(empty? a) b]
        [(empty? b) a]
        ; if b is in a, skip it
        [(in (car b) a) (union a (cdr b))]
        [else (cons (car b) (union a (cdr b)))])))

(define sum-of-squares
  (lambda (a)
    ; do x*x for every element in a, then sum them all up
    (apply + [map (lambda (x) (* x x)) a])))
        
    (list (equal? (union '(a b d e f h j) '(f c e g a)) '(c g a b d e f h j)) (equal? (sum-of-squares (list 1 3 5 7)) 84))
`
//export const TEST_PROG = `(cond [#f 1] [#f 2])`

export const TEST_PROG_BC = new AnimaCompiler().compileExpr(new ASP(TEST_PROG).parse(), false, false)
*/

/*
describe('Anima', () => {
    let evaluator: Anima = new Anima();
    let baseData: Record<string, any> = {
        port: 8080,
        protocol: "tcp",
        is_active: true,
        user_role: null 
    };

    const run = (expr: string) => {
        const ast = new ASP(expr).parse();
        return evaluator.evaluate(ast, baseData);
    };

    describe('Primitives, Strings & Symbols', () => {
        it('evaluates boolean primitives', () => {
            expect(run("#t")).toBe(true);
            expect(run("#f")).toBe(false);
        });

        it('evaluates numbers and raw arrays', () => {
            expect(run("42")).toBe(42);
            expect(run("()")).toBe(null); // ASP parses () as [], VM evals [] as null
        });

        it('evaluates native Symbols as implicit variables', () => {
            expect(run("port")).toBe(8080);
            expect(run("protocol")).toBe("tcp");
        });

        it('evaluates literal JS strings as Scheme strings directly', () => {
            expect(run('"hello"')).toBe("hello");
            expect(run('"port"')).toBe("port"); // String primitive, not a variable lookup
        });

        it('errors for unknown variables', () => {
            expect(() => run("missing_var")).toThrow(MissingVarError);
        });
    });

    describe('Math Operations', () => {
        it('performs basic arithmetic', () => {
            expect(run("(+ 10 5)")).toBe(15);
            expect(run("(- 10 5)")).toBe(5);
            expect(run("(* 10 5)")).toBe(50);
            expect(run("(/ 10 5)")).toBe(2);
            expect(run("(modulo 10 3)")).toBe(1);
        });

        it('performs numeric comparisons', () => {
            expect(run("(> port 1024)")).toBe(true);
            expect(run("(< port 10000)")).toBe(true);
            expect(run("(>= 10 10)")).toBe(true);
            expect(run("(<= 5 10)")).toBe(true);
        });
    });

    describe('Data Structures & Types', () => {
        it('creates lists and evaluates length', () => {
            expect(run("(list 1 2 3)")).toEqual([1, 2, 3]);
            expect(run(`(length (list "a" "b"))`)).toBe(2);
            expect(run(`(length "string_len")`)).toBe(10);
        });

        it('checks contains', () => {
            expect(run("(contains (list 1 2) 2)")).toBe(true);
            expect(run("(contains (list 1 2) 3)")).toBe(false);
        });

        it('evaluates type? with JS Symbols', () => {
            expect(run("(type? port)")).toBe("number");
            expect(run("(type? protocol)")).toBe("string");
            expect(run("(type? user_role)")).toBe("list");
            
            // Verifying our new native Symbol logic!
            expect(run("(type? 'my_symbol)")).toBe("symbol");
        });
    });

    describe('Lexical Scoping & Closures', () => {
        it('executes defines correctly', () => {
            const script = `
                (define x 10)
                (define y 20)
                (+ x y)
            `;
            expect(run(script)).toBe(30);
            expect(baseData.x).toBeUndefined(); // ensure it never pollutes outer scope
        });

        it('creates and calls a lambda with arguments', () => {
            const script = `
                (begin
                  (define (add a b) (+ a b))
                  (add 5 7))
            `;
            expect(run(script)).toBe(12);
        });

        it('creates and calls a lambda with multiple body expressions', () => {
            const script = `
                (begin
                  (define counter 0)
                  (define (increment)
                    (define counter (+ counter 1))
                    counter)
                  (increment))
            `;
            expect(run(script)).toBe(1);
        });

        it('respects closure scope (variables enclosed at creation)', () => {
            const script = `
                (begin
                  (define x 100)
                  (define (make_adder y) (+ x y))
                  (define x 999)
                  (make_adder 5))
            `;
            expect(run(script)).toBe(1004);
        });

        it('Ensure valid TCO', () => {
            const script = `
                (begin
                  (define (loop n)
                    (if (= n 0)
                        "survived!"
                        (loop (- n 1))))
                  (loop 15000))
            `;
            expect(() => run(script)).not.toThrow();
            expect(run(script)).toBe("survived!");
        });
    });

    describe('quote operator & Native Symbols', () => {
        it('quotes primitive numbers', () => {
            expect(run("'42")).toBe(42);
        });

        it('quotes Symbols perfectly', () => {
            expect(run("'x")).toBe(Symbol.for("x"));
        });

        it('protects lists and retains inner native types', () => {
            expect(run("'(+ 1 2)")).toEqual([Symbol.for("+"), 1, 2]);
        });

        it('protects nested lists perfectly', () => {
            expect(run("'(1 (2 3) 4)")).toEqual([1, [2, 3], 4]);
        });

        it('handles nested quotes recursively', () => {
            expect(run("''100")).toEqual([Symbol.for("quote"), 100]);
        });

        it('throws an error if given too many arguments natively', () => {
            expect(() => run("(quote 1 2)")).toThrow();
        });
    });

    describe('cond special form', () => {
        it('Matches the first truthy condition', () => {
            const script = `
                (cond 
                  (#t "first")
                  (#t "second"))
            `;
            expect(run(script)).toBe("first");
        });

        it('Skips falsey conditions and matches later ones', () => {
            const script = `
                (cond 
                  (#f "first")
                  ((= 1 2) "second")
                  ((> 5 3) "third"))
            `;
            expect(run(script)).toBe("third");
        });

        it('Falls back to the else clause if nothing matches', () => {
            const script = `
                (cond 
                  (#f "first")
                  (#f "second")
                  (else "fallback"))
            `;
            expect(run(script)).toBe("fallback");
        });

        it('Returns void if no conditions match and there is no else clause', () => {
            const script = `
                (cond 
                  (#f "first")
                  ((= 1 2) "second"))
            `;
            expect(run(script)).toBeUndefined();
        });
    });

    describe('Cons, Arrays & FFI Boundary', () => {
        it('constructs a proper list and extracts values (car/cdr)', () => {
            expect(run("(car (cons 1 (cons 2 null)))")).toBe(1);
            expect(run("(car (cdr (cons 1 (cons 2 null))))")).toBe(2);
        });

        it('supports improper lists perfectly', () => {
            expect(run("(car (cons 1 2))")).toBe(1);
            expect(run("(cdr (cons 1 2))")).toBe(2);
            expect(run("(length (cons 1 2))")).toBe(1);
        });

        it('triggers the O(1) Fast Path for raw JS arrays', () => {
            expect(run("(car '(10 20 30))")).toBe(10);
            expect(run("(car (cdr '(10 20 30)))")).toBe(20);
            expect(run("(length '(10 20 30))")).toBe(3);
        });

        it('handles array -> Cons perfectly', () => {
            expect(run("(car (cons 1 '(2 3)))")).toBe(1);
            expect(run("(car (cdr (cons 1 '(2 3))))")).toBe(2);
            expect(run("(length (cons 1 '(2 3)))")).toBe(3); 
        });

        it('throws errors for empty lists', () => {
            expect(() => run("(car null)")).toThrow();
            expect(() => run("(car '())")).toThrow();
            expect(() => run("(cdr '())")).toThrow();
        });

        it('traverses cons-array hybrids with standard operators (last, contains)', () => {
            expect(run(`(contains (cons "a" (cons "b" null)) "b")`)).toBe(true);
            expect(run(`(contains (cons "a" (cons "b" null)) "c")`)).toBe(false);
            expect(run(`(contains '("a" "b") "b")`)).toBe(true);

            expect(run(`(last (cons "a" (cons "b" null)))`)).toBe("b");
            expect(run(`(last '("a" "b"))`)).toBe("b");
            expect(run(`(last (cons "a" "b"))`)).toBe("b");
        });
    });

    describe('let bindings', () => {
        it('evaluates a simple single binding', () => {
            const script = `(let ([x 10]) x)`;
            expect(run(script)).toBe(10);
        });

        it('evaluates multiple bindings', () => {
            const script1 = `
                (let ([x 10] [y 20] [z 5]) 
                  (- (+ x y) z))
            `;
            expect(run(script1)).toBe(25);

            const script2 = `(let ((a 5) (b 5)) (+ a b))`;
            expect(run(script2)).toBe(10);
        });

        it('shadows outer variables without mutating them (Lexical Purity)', () => {
            const script = `
                (begin
                  (define x 100)
                  (define result (let ([x 5] [y 5]) (+ x y)))
                  (list result x))
            `;
            expect(run(script)).toStrictEqual([10, 100]);
        });

        it('supports multiple body expressions', () => {
            const script = `
                (let ([multiplier 10])
                  (define x 5)
                  (define y 2)
                  (* x y multiplier))
            `;
            expect(run(script)).toBe(100);
        });

        it('handles empty bindings correctly', () => {
            const script = `(let () 99)`;
            expect(run(script)).toBe(99);
        });
        
        it('throws an error for malformed bindings', () => {
            const script = `(let ([x]) x)`;
            expect(() => run(script)).toThrow();
        });
    });

    describe('apply', () => {
        it('applies a procedure to a single list of arguments', () => {
            expect(run(`(apply + '(1 2 3 4))`)).toBe(10);
            expect(run(`(apply * '(2 3 4))`)).toBe(24);
        });

        it('handles preceding standalone arguments before the final list', () => {
            expect(run(`(apply + 100 200 '(1 2))`)).toBe(303);
            expect(run(`(apply - 100 '(50 25))`)).toBe(25);
        });

        it('works flawlessly with user-defined closures', () => {
            const script = `
                (begin
                  (define (multiply-add a b c) (+ (* a b) c))
                  (apply multiply-add 10 '(5 2)))
            `;
            // (10 * 5) + 2 = 52
            expect(run(script)).toBe(52); 
        });

        it('handles the empty list gracefully', () => {
            const script = `
                (begin
                  (define (return-five) 5)
                  (apply return-five '()))
            `;
            expect(run(script)).toBe(5);
        });

        it('throws an error if the last argument is not a list', () => {
            expect(() => run(`(apply + 1 2 3)`)).toThrow(/must be a list/);
        });
    });

    describe('map', () => {
        it('maps a procedure over a single list', () => {
            const script = `
                (begin
                  (define (double x) (* x 2))
                  (map double '(1 2 3 4)))
            `;
            expect(run(script)).toEqual([2, 4, 6, 8]);
        });

        it('maps a procedure over multiple lists in parallel', () => {
            const script = `(map + '(1 2 3) '(10 20 30))`;
            expect(run(script)).toEqual([11, 22, 33]);
            
            const script3 = `(map + '(1 1 1) '(2 2 2) '(3 3 3))`;
            expect(run(script3)).toEqual([6, 6, 6]);
        });

        it('safely terminates when the shortest list is exhausted', () => {
            const script = `(map + '(1 2 3 4 5) '(10 20))`;
            expect(run(script)).toEqual([11, 22]);
        });

        it('maps over Cons as well', () => {
            const script = `(map + (cons 1 (cons 2 null)) '(10 20))`;
            expect(run(script)).toEqual([11, 22]);
        });

        it('returns an empty list when mapping over an empty list', () => {
            const script = `(map + '())`;
            expect(run(script)).toEqual([]);
        });

        it('throws an error if given a non-list argument', () => {
            expect(() => run(`(map + '(1 2 3) 5)`)).toThrow(/must be lists/);
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

        expect(run(`
(define union
    (lambda (a b)
        (define (in a rst) 
        (cond 
            [(empty? rst) #f]
            [(equal? a (car rst)) #t]
            [else (in a (cdr rst))]))

        (cond
        ; if either set is empty, the other one if the union
        [(empty? a) b]
        [(empty? b) a]
        ; if b is in a, skip it
        [(in (car b) a) (union a (cdr b))]
        [else (cons (car b) (union a (cdr b)))])))

(define sum-of-squares
  (lambda (a)
    ; do x*x for every element in a, then sum them all up
    (apply + [map (lambda (x) (* x x)) a])))
        
    (list (equal? (union '(a b d e f h j) '(f c e g a)) '(c g a b d e f h j)) (equal? (sum-of-squares (list 1 3 5 7)) 84))
        `)).toEqual("(#t #t)")
        });
    });
});
*/