import type { MSyscallArgs, MSyscallError, MSyscallRet } from "./syscall"
import type { Id } from "./types/common";
import { expand, decode, type KhronosValue, type CKhronosValue } from "./khronosvalue";

export class Result<T, E> {
    #inner: { ok: true, res: T } | { ok: false, res: E }
    constructor(inner: { ok: true, res: T } | { ok: false, res: E }) {
        this.#inner = inner
    }

    get ok() { return this.#inner.ok }

    /**
     * Unwraps the contained ok-value
     */
    unwrap(): T {
        if (this.#inner.ok) {
            return this.#inner.res
        } else {
            throw new Error("internal error: unwrap() called on error value")
        }
    }

    /**
     * Unwraps the contained ok-value
     */
    unwrapErr(): E {
        if (!this.#inner.ok) {
            return this.#inner.res
        } else {
            throw new Error("internal error: unwrapErr() called on ok value")
        }
    }

    /**
     * Maps the ok (if present) to the desired ok state
     */
    map<U>(f: (ok: T) => U): Result<U, E> {
        if (this.#inner.ok) {
            return new Result({ok: true, res: f(this.#inner.res)}) as unknown as Result<U, E>
        } else {
            return new Result({ok: false, res: this.#inner.res}) as unknown as Result<U, E>
        }
    }

    /**
     * Maps the error (if present) to the desired error
     */
    mapErr<U>(f: (err: E) => U): Result<T, U> {
        if (!this.#inner.ok) {
            return new Result({ok: false, res: f(this.#inner.res)}) as unknown as Result<T, U>
        } else {
            return new Result({ok: true, res: this.#inner.res}) as unknown as Result<T, U>
        }
    }

    /**
     * Maps the error (if present) to the desired error
     */
    stringifyAndThrow(f: (err: E) => string): T {
        if (!this.#inner.ok) {
            throw new Error(f(this.#inner.res))
        } else {
            return this.#inner.res
        }
    }
}  

export const msyscall = async (instanceUrl: string, auth: string | undefined, call: MSyscallArgs): Promise<Result<MSyscallRet, MSyscallError>> => {
    try {
        const resp = await fetch(`${instanceUrl}/msyscall`, {
            method: "POST",
            headers: auth ? {
                "Authorization": auth,
                "Content-Type": "application/json"
            } : {
                "Content-Type": "application/json"
            },
            body: JSON.stringify(call)
        })
        if(!resp.ok) {
            if (resp.status === 429) {
                let retryAfter = resp.headers.get("Retry-After")
                if(retryAfter) {
                    let secs = parseFloat(retryAfter)
                    if(!isNaN(secs)) {
                        console.error("Ratelimited, waiting...")
                        await new Promise((resolve) => setTimeout(resolve, secs * 1000));
                        return await msyscall(instanceUrl, auth, call)
                    }
                }
            }

            const json = await resp.json()
            return new Result({ok: false, res: json})
        }
        const json = await resp.json()
        return new Result({ok: true, res: json})
    } catch (err) {
        let e: MSyscallError = {op: "Generic", message: err?.toString() || "Unknown error"}
        return new Result({ok: false, res: e as any})
    }
}

export const errorString = (err: MSyscallError): string => {
    switch (err.op) {
        case "Generic":
            return err.message;
        case "InvalidEvent":
            return `Invalid event: ${err.reason}`;
        case "ContextInsecure":
            return "The current API context is too insecure to perform this operation (admin only)";
        case "ContextRequiresUser":
            return "This operation requires a logged-in user.";
        case "ContextRequiresOauth":
            return "This operation requires an OAuth2 session.";
        case "BotNotOnGuild":
            return "The bot is not present in the specified guild.";
        case "UserOauth2Needed":
            return "You need to log in via OAuth2 at least once to use this API.";
        case "AuthError":
            switch (err.reason.op) {
                case "InvalidRedirectUri":
                    return "Invalid redirect URI.";
                case "CodeTooShort":
                    return "Authorization code is too short.";
                case "CodeReuseDetected":
                    return "Authorization code has already been used.";
                case "NeededScopesNotFound":
                    return "Required OAuth2 scopes (identify, guilds) were not found.";
                case "ExpiryTimeOutOfRange":
                    return "Session expiry time is out of range.";
                default:
                    return `Authentication error: ${(err.reason as any).op}`;
            }
        case "Unauthorized":
            return `Unauthorized: ${err.reason}`;
        case "EntityNotFound":
            return `Not found: ${err.reason}`;
        case "Ratelimited":
            return `Ratelimited on bucket ${err.bucket}, requesting bucket of ${err.req_bucket} for ${err.retry_after} seconds`
    }
}

export class FeedClient {
    private ws: WebSocket | null = null;
    private shouldReconnect = true;
    private reconnectTimer: number | null = null;

    constructor(
        private instanceUrl: string,
        private auth: string | undefined,
        private id: Id,
        private topics: string[],
        private onMessage: (topic: string, msg: KhronosValue) => void,
        private onStatusChange: (connected: boolean) => void
    ) {
        this.connect();
    }

    private async connect() {
        if (!this.shouldReconnect) return;
        
        // 1. Obtain a feedticket from the backend
        const res = await msyscall(this.instanceUrl, this.auth, {
            op: "Bot",
            req: {
                op: "FeedTicket",
                id: this.id,
                topics: this.topics
            }
        });
        
        if (!res.ok) {
            console.error("Failed to get feedticket for stream:", res.unwrapErr());
            this.scheduleReconnect();
            return;
        }
        
        const data = res.unwrap();
        if (data.op !== "Bot" || data.data.op !== "FeedTicket") {
            console.error("Invalid response for FeedTicket");
            this.scheduleReconnect();
            return;
        }
        
        const { payload, sig } = data.data;
        
        // 2. Build WebSocket URL
        const wsUrl = new URL("/ws", this.instanceUrl);
        wsUrl.protocol = wsUrl.protocol === "https:" ? "wss:" : "ws:";
        wsUrl.searchParams.set("p", payload);
        wsUrl.searchParams.set("s", sig);
        
        const ws = new WebSocket(wsUrl.toString());
        this.ws = ws;
        
        ws.onopen = () => {
            this.onStatusChange(true);
        };
        
        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                if (msg.Hb) {
                    // Send back heartbeat to keep alive
                    ws.send(JSON.stringify({ Hb: {} }));
                } else if (msg.Feed) {
                    // Decode Feed message (server to client)
                    const expanded = expand(msg.Feed.msg);
                    const decoded = decode(expanded);
                    this.onMessage(msg.Feed.topic, decoded);
                }
            } catch (err) {
                console.error("Failed to parse WS message", err);
            }
        };
        
        ws.onclose = () => {
            if (this.ws === ws) {
                this.onStatusChange(false);
                this.ws = null;
                this.scheduleReconnect();
            }
        };
        
        ws.onerror = (err) => {
            console.error("WS error:", err);
            // The socket will eventually close and trigger onclose for reconnects
        };
    }
    
    private scheduleReconnect() {
        if (!this.shouldReconnect) return;
        if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
        // Exponential backoff or static 3s could be used. We'll use 3s.
        this.reconnectTimer = setTimeout(() => this.connect(), 3000) as unknown as number;
    }
    
    public send(_msg: CKhronosValue) {
        throw new Error("Feed is read-only");
    }
    
    public destroy() {
        this.shouldReconnect = false;
        if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
    }
}

