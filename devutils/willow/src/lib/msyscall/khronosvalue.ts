export type RawKhronosValue = {
    Text: string;
} | {
    Integer: number;
} | {
    Int64: string;
} | {
    Float: number;
} | {
    Boolean: boolean;
} | {
    Vector: [number, number, number];
} | {
    Map: [RawKhronosValue, RawKhronosValue][];
} | {
    StrMap: Record<string, RawKhronosValue>, // strmap optimization
} | {
    List: RawKhronosValue[];
} | {
    Timestamptz: string; // ISO 8601 format
} | {
    Interval: [number, number]; // [seconds, nanoseconds]
} | {
    TimeZone: string; // Time zone identifier
} | {
    MemoryVfs: Record<string, string>;
} | {
    Null: null
} | {
    Nil: null
}


export class MemoryVfs {
    public map: Record<string, string>;

    constructor(map: Record<string, string>) {
        this.map = map;
    }
}

export class Vector {
    public vector: [number, number, number];
    
    constructor(vector: [number, number, number]) {
        this.vector = vector;
    }
}

export class Interval {
    public seconds: number;
    public nanoseconds: number;

    constructor(seconds: number, nanoseconds: number) {
        this.seconds = seconds;
        this.nanoseconds = nanoseconds;
    }

    /** Helper to get total milliseconds (for JS timeouts, Date math, etc.) */
    public toMilliseconds(): number {
        return (this.seconds * 1000) + Math.floor(this.nanoseconds / 1_000_000);
    }
}

export class TimeZone {
    public timezone: string;

    constructor(timezone: string) {
        this.timezone = timezone;
    }
}

export type KhronosValue = 
    | string 
    | number 
    | bigint 
    | boolean 
    | null 
    | undefined 
    | Vector 
    | Date 
    | Interval 
    | TimeZone 
    | MemoryVfs 
    | Map<KhronosValue, KhronosValue> 
    | KhronosValue[];

/**
 * Decode a RawKhronosValue into a nicer to work with JavaScript value
 * @param data The data from the server
 */
export const decode = (data: RawKhronosValue, depth?: number): KhronosValue => {
    if ((depth || 0) > 100) {
        return null; // Prevent excessive recursion
    }
    
    if ('Text' in data) {
        return data.Text;
    } else if ('Integer' in data) {
        return data.Integer;
    } else if ('Int64' in data) {
        return BigInt(data.Int64);
    } else if ('Float' in data) {
        return data.Float;
    } else if ('Boolean' in data) {
        return data.Boolean;
    } else if ('Vector' in data) {
        return new Vector(data.Vector);
    } else if ('Map' in data) {
        const obj: Map<KhronosValue, KhronosValue> = new Map();
        for (const [key, value] of data.Map) {
            const decodedKey = decode(key, (depth || 0) + 1);
            const decodedValue = decode(value, (depth || 0) + 1);
            obj.set(decodedKey, decodedValue);
        }
        return obj;
    } else if ('StrMap' in data) {
        const obj: Map<KhronosValue, KhronosValue> = new Map();
        for (const [key, value] of Object.entries(data.StrMap)) {
            const decodedValue = decode(value, (depth || 0) + 1);
            obj.set(key, decodedValue);
        }
        return obj;
    } else if ('List' in data) {
        return data.List.map((item) => decode(item, (depth || 0) + 1));
    } else if ('Timestamptz' in data) {
        return new Date(data.Timestamptz);
    } else if ('Interval' in data) {
        return new Interval(data.Interval[0], data.Interval[1]);
    } else if ('TimeZone' in data) {
        return new TimeZone(data.TimeZone);
    } else if ('MemoryVfs' in data) {
        return new MemoryVfs(data.MemoryVfs);
    } else if ('Nil' in data) {
        return undefined
    } else if ('Null' in data) {
        return null
    } else {
        throw new Error('Unknown KhronosValue type');
    }
}

export type EncodableKhronosValue = 
    | string 
    | number 
    | bigint 
    | boolean 
    | null 
    | undefined 
    | Vector 
    | Date 
    | Interval 
    | TimeZone 
    | MemoryVfs 
    | Map<EncodableKhronosValue, EncodableKhronosValue> 
    | EncodableKhronosValue[] 
    | { [key: string]: EncodableKhronosValue };

/**
 * Encode a JavaScript value into a KhronosValue, unknown types are encoded using toString()
 * @param value The Value to encode into a KhronosValue
 */
export const encode = (value: EncodableKhronosValue): RawKhronosValue => {
    if (value === null) {
        return { Null: null };
    } else if (value === undefined) {
        return { Nil: null }
    } else if (typeof value === 'string') {
        return { Text: value };
    } else if (typeof value === 'number') {
        if (Number.isInteger(value)) {
            return { Integer: value };
        } else {
            return { Float: value };
        }
    } else if (typeof value == "bigint") {
        return { Int64: value.toString() }
    } else if (typeof value === 'boolean') {
        return { Boolean: value };
    } else if (value instanceof Vector) {
        return { Vector: value.vector };
    } else if (Array.isArray(value)) {
        return { List: value.map(encode) };
    } else if (value instanceof MemoryVfs) {
        return { MemoryVfs: value.map };
    } else if (value instanceof Date) {
        return { Timestamptz: value.toISOString() };
    } else if (value instanceof Interval) {
        return { Interval: [value.seconds, value.nanoseconds] };
    } else if (value instanceof TimeZone) {
        return { TimeZone: value.timezone };
    } else if (value instanceof Map) {
        let isPureStrMap = true;
        const strMap: Record<string, RawKhronosValue> = {};
        const genericMap: [RawKhronosValue, RawKhronosValue][] = [];

        for (const [key, val] of value.entries()) {
            const encodedVal = encode(val);

            if (isPureStrMap) {
                if (typeof key === "string") {
                    strMap[key] = encodedVal;
                    continue; 
                } else {
                    // We hit a non-string key! 
                    // Drain everything we've built so far into the generic map
                    isPureStrMap = false;
                    for (const strKey in strMap) {
                        genericMap.push([{ Text: strKey }, strMap[strKey]]);
                    }
                }
            }

            // If we are no longer a pure string map, push to the generic map array
            genericMap.push([encode(key), encodedVal]);
        }

        if (isPureStrMap) {
            return { StrMap: strMap };
        } else {
            return { Map: genericMap };
        }
    } else if (value !== null && typeof value === 'object' && value.constructor === Object) {
        // Fallback for simple objects
        const mapEntries: Record<string, RawKhronosValue> = {};
        for (const [key, val] of Object.entries(value)) {
            // Objects only allow string keys
            mapEntries[key] = encode(val);
        }
        return { StrMap: mapEntries };
    } else {
        throw new Error("unknown object passed to encode()")
    }
}

/**
 * Compressed Khronos Value format (CKhronosValue)
 */
export type CKhronosValue = | string
  | null // Nil
  | boolean // Boolean
  | CKhronosValue[] // List
  // tagged
  | { I: number } // Integer
  | { F: number } // Float
  | { I64: string } // Int64 (BigInt in js)
  | { N: null } // Null
  | { "#SM": Record<string, CKhronosValue> }  // StrMap
  | { M: [CKhronosValue, CKhronosValue][] } // Map
  | { Vec: [number, number, number] }     // Vector 
  | { TS: string }                        // Timestamptz 
  | { TZ: string }                        // TimeZone 
  | { Interval: [number, number]; }       // Interval [seconds, nanoseconds]
  | { MVfs: Record<string, string> };     // MemoryVfs

export const expand = (comp: CKhronosValue): RawKhronosValue => {
    if (comp === null) return { Nil: null }
    else if (typeof comp === "string") return {Text: comp}
    else if (typeof comp === "boolean") return {Boolean: comp}
    else if (Array.isArray(comp)) return {List: comp.map(expand)}
    else if ('I' in comp) return {Integer: comp.I}
    else if ('F' in comp) return {Float: comp.F}
    else if ('I64' in comp) return {Int64: comp.I64}
    else if ('N' in comp) return {Null: null}
    else if ('#SM' in comp) return {StrMap: Object.fromEntries(Object.entries(comp["#SM"]).map(([k, v]) => [k, expand(v)]))}
    else if ('M' in comp) return {Map: comp.M.map(([a, b]) => [expand(a), expand(b)])}
    else if ('Vec' in comp) return {Vector: comp.Vec}
    else if ('TS' in comp) return {Timestamptz: comp.TS}
    else if ('TZ' in comp) return {TimeZone: comp.TZ}
    else if ('Interval' in comp) return {Interval: comp.Interval}
    else if ('MVfs' in comp) return {MemoryVfs: comp.MVfs}
    else throw new Error('Unknown CKhronosValue type in expand()');
}

export const dexpand = (comp: RawKhronosValue): CKhronosValue => {
    //else if (typeof comp === "string") return {Text: comp}
    //else if (typeof comp === "boolean") return {Boolean: comp}
    //else if (Array.isArray(comp)) return {List: comp.map(expand)}
    
    if ('Text' in comp) return comp.Text
    else if ('Boolean' in comp) return comp.Boolean
    else if ('List' in comp) return comp.List.map(dexpand)
    else if ('Integer' in comp) return {I: comp.Integer}
    else if ('Float' in comp) return {F: comp.Float}
    else if ('Int64' in comp) return {I64: comp.Int64}
    else if ('Null' in comp) return {N: null}
    else if ('Nil' in comp) return null
    else if ('StrMap' in comp) return {"#SM": Object.fromEntries(Object.entries(comp.StrMap).map(([k, v]) => [k, dexpand(v)]))}
    else if ('Map' in comp) return {M: comp.Map.map(([a, b]) => [dexpand(a), dexpand(b)])}
    else if ('Vector' in comp) return {Vec: comp.Vector}
    else if ('Timestamptz' in comp) return {TS: comp.Timestamptz}
    else if ('TimeZone' in comp) return {TZ: comp.TimeZone}
    else if ('Interval' in comp) return {Interval: comp.Interval}
    else if ('MemoryVfs' in comp) return {MVfs: comp.MemoryVfs}
    else throw new Error('Unknown RawKhronosValue type in dexpand()');
}