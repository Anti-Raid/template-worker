/**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KhronosValue {
    Text(String),
    Integer(i64),
    Int64(#[serde(with = "string_i64")] i64),
    Float(f64),
    Boolean(bool),
    Buffer(Vec<u8>),   // Binary data
    Vector((f32, f32, f32)), // Luau vector
    Map(Vec<(KhronosValue, KhronosValue)>),
    List(Vec<KhronosValue>),
    Timestamptz(chrono::DateTime<chrono::Utc>),
    Interval(chrono::Duration),
    TimeZone(chrono_tz::Tz),
    MemoryVfs(HashMap<String, String>),
    Null,
}
 */

type RawKhronosValue = {
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
    Buffer: number[];
} | {
    Vector: [number, number, number];
} | {
    Map: [RawKhronosValue, RawKhronosValue][];
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
    Null: null;
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
    | Uint8Array 
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
    } else if ('Buffer' in data) {
        return new Uint8Array(data.Buffer);
    } else if ('Vector' in data) {
        return new Vector(data.Vector);
    } else if ('Map' in data) {
        const obj: Map<any, any> = new Map();
        for (const [key, value] of data.Map) {
            const decodedKey = decode(key, (depth || 0) + 1);
            const decodedValue = decode(value, (depth || 0) + 1);
            obj.set(decodedKey, decodedValue);
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
    } else if ('Null' in data) {
        return null;
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
    | Uint8Array 
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
    if (value === null || value === undefined) {
        return { Null: null };
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
    } else if (value instanceof Uint8Array) {
        return { Buffer: Array.from(value) };
    } else if (value instanceof Vector) {
        return { Vector: value.vector };
    } else if (Array.isArray(value)) {
        return { List: value.map((item) => encode(item)) };
    } else if (value instanceof MemoryVfs) {
        return { MemoryVfs: value.map };
    } else if (value instanceof Date) {
        return { Timestamptz: value.toISOString() };
    } else if (value instanceof Interval) {
        return { Interval: [value.seconds, value.nanoseconds] };
    } else if (value instanceof TimeZone) {
        return { TimeZone: value.timezone };
    } else if (value instanceof Map) {
        const mapEntries: [RawKhronosValue, RawKhronosValue][] = [];
        for (const [key, val] of value.entries()) {
            mapEntries.push([encode(key), encode(val)]);
        }
        return { Map: mapEntries };
    } else if (value !== null && typeof value === 'object' && value.constructor === Object) {
        // Fallback for simple objects
        const mapEntries: [RawKhronosValue, RawKhronosValue][] = [];
        for (const [key, val] of Object.entries(value)) {
            // Objects only allow string keys
            mapEntries.push([{ Text: key }, encode(val)]);
        }
        return { Map: mapEntries };
    } else {
        throw new Error("unknown object passed to encode()")
    }
}

// test
const json: RawKhronosValue = {
  "Map": [
    [
      {
        "Text": "key1"
      },
      {
        "Text": "value1"
      }
    ],
    [
      {
        "Text": "key2"
      },
      {
        "Vector": [
          1.0,
          2.1,
          3.2
        ]
      }
    ],
    [
      {
        "Text": "key3"
      },
      {
        "MemoryVfs": {}
      }
    ]
  ]
}


console.log(decode(json));
console.log(encode(decode(json)));