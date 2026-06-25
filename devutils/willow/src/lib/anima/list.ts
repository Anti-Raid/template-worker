export class Cons {
    private constructor(
        private readonly _isView: boolean, // if our cons represents a view to an array or not
        private readonly _head: any,
        private readonly _tail: Cons | null,
        private readonly _array: any[] | null,
        private readonly _offset: number,
        public readonly length: number
    ) {}

    // construct a non-view cons representing a linked list pair
    static pair(head: any, tail: any): Cons {
        // ensure any array tail becomes a Cons
        let rtail = tail;
        if (Array.isArray(tail)) {
            rtail = Cons.fromArray(tail);
        }

        const len = 1 + (rtail instanceof Cons ? rtail.length : 0);
        return new Cons(false, head, rtail, null, 0, len);
    }

    // construct a cons with an array w/ a given offset into said array
    static fromArray(arr: any[], offset: number = 0): Cons | null {
        if (offset >= arr.length) return null;
        
        const len = arr.length - offset;
        return new Cons(true, null, null, arr, offset, len);
    }

    get head(): any {
        return this._isView ? this._array![this._offset] : this._head;
    }

    get tail(): Cons | null {
        if (this._isView) {
            return Cons.fromArray(this._array!, this._offset + 1);
        }
        return this._tail;
    }

    *[Symbol.iterator]() {
        let current: any = this;
        
        while (current !== null) {
            if (current instanceof Cons) {
                if (current._isView) {
                    const arr = current._array!;
                    if (current._offset === 0) {
                        yield* arr;
                        break
                    } else {
                        for(let i = current._offset; i < arr.length; i++) {
                            yield arr[i]
                        }
                        break
                    }
                }
                yield current.head;
                current = current.tail;
            } else {
                // improper lists are annoying and need to know the final element is improper
                return current
            }
        }
    }

    includes(elem: any): boolean {
        for(const e of this) {
            if (e === elem) {
                return true
            }
        }
        return false;
    }
    
    get(idx: number): any {
        if (idx < 0 || idx >= this.length) {
            return undefined;
        }

        let current: Cons | null = this;
        let stepsRemaining = idx;

        while (current !== null) {
            if (current._isView) { // array view terminator
                return current._array![current._offset + stepsRemaining];
            }

            if (stepsRemaining === 0) {
                return current._head;
            }

            current = current._tail;
            stepsRemaining--;
        }

        return undefined;
    }
}

/*
const c = Cons.pair(-1, Cons.pair(0, Cons.fromArray([1,2,3])))
console.log(c.length)
for(const d of c) {
    console.log(d)
}
console.log(c.includes(-1))
console.log(c.includes(1))
console.log(c.includes(6))
console.log("cg",c.get(3))

const c2 = Cons.pair(-1, Cons.pair(0, Cons.fromArray([1,2,3], 1)))
console.log(c2.length)
for(const d of c2) {
    console.log(d)
}
console.log(c2.includes(-1))
console.log(c2.includes(1))
console.log(c2.includes(6))
console.log("cg", c2.get(3))
*/