# Anima Language

Anima is the custom (Scheme-inspired) language used in settings v2 in antiraid for dynamic branching + complex client-side validation etc.

## Specification

### Grammar

Anima uses a (simplified) Scheme-like grammar based on 's-expressions'

```
<program>   ::= <expr>* 
<expr>      ::= <primitive> | <list> | <quoted>
<primitive> ::= <null> | <boolean> | <number> | <string> | <symbol>
<null>    ::= null
<boolean> ::= true | false
<number>  ::= [0-9]+ ("." [0-9]+)? 
<string>  ::= "[json/lisp escaped string]"
<special> ::= ( | ) | [ | ] | ; | " | '
<symbol>  ::= [character (excluding <special> and whitespace)]+
<list>    ::= (<expr>*) | [<expr>*]
<quoted>  ::= '<expr>
```

Comments begin with ; and continue to the end of the line.

### Execution

- Strings and symbols are distinct data types. String literals (e.g., "hello") evaluate to themselves as string primitives
- Unquoted symbols (e.g. my-var) are evaluated as dynamic variable lookups in the lexical scope.
- A quoted expression like '<expr> should have the same effect as (quote <expr>). Quoting a symbol (e.g., 'my-var) returns an interned 
symbol rather than performing a variable lookup.
- Multiple top-level expressions should be evaluated sequentially with the result being the result of the last expression (one way
to achieve this is to parse multiple top-level expressions expr1 expr2... in a begin block like (begin expr1 expr2 ...))

### Other Rules

1. Like Scheme, Anima makes use of lexical scoping. Nested scopes inherit parent variables and can 'shadow' parent variables of the same name.
`define` strictly mutates or initializes within the local execution scope and never the parent scope and variables in the outermost scope 
cannot be reassigned or mutated whatsoever for sandboxing purposes.

2. Truthiness: false and null are falsey. All other values are truthy.

3. Tail-Call Optimization (TCO): The runtime must execute the final expression in begin, if, and, or, and custom procedure calls without allocating a new frame on the host call stack.

4. Anima does not support macros/custom syntax extensions.

5. It is not allowed for user-code to override a builtin using define. Compliant implementations of Anima should error if an attempt to do so is detected

6. Like Scheme, all procedures in Anima (including builtin procedures that are *not* special forms) must be first class. Furthermore, both builtin
and user-defined procedures must return `procedure` if type? is called on it.

7. Compliant implementations of Anima should support the following options:
- disableLambda (disables creation of closures using lambda)
- disableDefine (disables setting values to scope to define)
- maxSteps (the maximum number of steps that anima code is allowed to use up. Each vm loop should use 1 step and future functions defined in the spec may
be defined to take up additional steps in addition to the fixed 1 step cost)

### Supported Features/Builtins

Implementations must enforce strict arity and type checking, throwing errors on mismatch.

### Special Forms

- define (varname: symbol, value: expr): Evaluates value and binds it to varname in the current scope. Arity: 2.
- define ((varname, args) body: expr): Creates a function (following same rules as lambda) with args and body as params+body then binds
it to varname in the current scope
- quote (expr): Returns the expression without evaluating it. Any raw identifiers within the quoted expression (or deeply nested 
within quoted lists) are converte into symbols. Arity: 1.
- lambda ([params...], body: expr): Returns a closure capturing the current lexical scope. If multiple body expressions are provided, 
they are evaluated sequentially with the last result returned Arity: >= 2.
- if (cond, true_expr, false_expr): Evaluates cond. If truthy, evaluates and returns true_expr, else false_expr. Arity: 3.
- cond (clauses...): Each clause must be a list of exactly two elements: [condition, expr]. Each condition must be executed in 
order. Upon encountering the first truthy clause condition (or the exact symbol 'else'), expr is evaluated and returned. If no 
conditions match, returns null. Throws an error if zero clauses are provided or if any clause is malformed. Arity: >= 1.
- and (expr...): Short-circuits on the first falsey evaluation. Returns true if 0 arguments.
- or (expr...): Short-circuits on the first truthy evaluation. Returns false if 0 arguments.
- begin (expr...): Evaluates arguments sequentially. Returns the result of the last expression. Arity: >= 1.
- 

### Builtin procedures

#### List Operations
- list (expr...): Evaluates arguments and returns them as a native array. Arity: >= 0.
- cons (a d): Returns a new list with a as head and d as tail. Arity: 2
- car (list): Returns the first element of the list. Throws if the list is empty. Arity: 1.
- cdr (list): Returns a new list excluding the first element. Throws if the list is empty. Arity: 1.
- last (list): Returns the final element of the list. Throws if the list is empty. Arity: 1.
- length (list | string): Returns the integer length. Returns 0 if the argument is neither a list nor string. Arity: 1.
- contains (list, item): Returns a boolean indicating strict inclusion of item within list. Arity: 2.

#### Logic & Type Checking
- =: Checks if `n` number expressions are equal. Errors if any expression is not a number. Arity: >= 2.
- eq? + eqv?: Similar to Scheme's eqv?. If number/string, checks equality of num/string even if in different memory locations, eqv? should be ``Object.is``-style
(like Scheme) but eq? can do just ``===``
otheriwse checks pointer equality. Arity: 2
- equal?: Similar to Scheme's equal? but does a deep recursive comparison for lists etc.
- not: Returns true if the expression is falsy, otherwise returns false
- <, >, <=, >= (expr1, expr2, ... exprN): Strict comparison between expr1 to exprN. Arity: >= 2.
- type? (expr): Returns one of the following strings: "list", "string", "number", "boolean", "null", "symbol", "procedure", "list", "error", "exposed-prop". Arity: 1.
- error? (expr): Returns if ``expr`` is an ``ErrorObject`` or not
- error-message (expr): Returns the underlying error caught within ``expr`` if ``expr`` is an ``ErrorObject``. Throws an error if ``expr`` is not a ``ErrorObject``

#### Mathematics
- +, -, *, / (expr...): Evaluates sequentially from left to right. Arity: >= 2.
- modulo (expr1, expr2): Returns the mathematical remainder. Arity: 2.

#### Misc
- apply (proc args... rem-lst): Same as Scheme specification on apply. Calls ``proc`` with the packed ``args... rem-lst`` as arguments for ``proc`` 
- try (proc args... rem-lst): Calls ``proc`` with the packed ``args... rem-lst`` as arguments for ``proc``. If ``proc`` errors, ``try`` evaluates to an
``res`` of type ``ErrorObject`` (whose ``type? res`` is ``error``, ``error? res`` yielding ``#t`` and ``error-message res`` containing the error caught by ``try`` while evaluating ``proc``)
- map: Same as Scheme specification on map (TODO: Write docs here for this as well)

## Deviations from Scheme

### No first-class continuations

In order to keep Anima simple to implement (and debug!), Anima does not support full first-class continuations *yet* (such as ``call-with-current-continuation`` or ``call/cc``) (although support for this may be implemented later at some point in the future). This also enables for potential future optimizations.