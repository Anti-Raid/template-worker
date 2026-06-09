import { type PartialGlobalKv } from '../types/gkv'

export type MGkvSyscall = 
  | { 
      /** Find global key-value entries based on a search query */
      op: "FindGlobalKvs"; 
      /** The scope to search in */
      scope: string; 
      /** The search query for the key. Supports SQL LIKE syntax (% for multiple chars, _ for one char) */
      query: string 
    }
  | { 
      /** Get a specific global key-value entry */
      op: "GetGlobalKv"; 
      /** The scope of the entry */
      scope: string; 
      /** The key of the entry */
      key: string; 
      /** The version of the entry */
      version: number 
    }
  | { 
      /** Set the review state for a global key-value entry (Secure only) */
      op: "AdminSetGlobalKvReviewState"; 
      /** The key of the entry */
      key: string; 
      /** The version of the entry */
      version: number; 
      /** The scope of the entry */
      scope: string; 
      /** The new review state (e.g., 'approved', 'pending') */
      review_state: string 
    };

export type MGkvSyscallRet = 
  | { 
      /** List of global key-value entries response */
      op: "GlobalKvList"; 
      /** The list of found partial global KV entries */
      gkvs: PartialGlobalKv[] 
    }
  | { 
      /** Single global key-value entry response */
      op: "GlobalKv"; 
      /** The requested partial global KV entry */
      gkv: PartialGlobalKv 
    }
  | { 
      /** Generic success acknowledgement */
      op: "Ack" 
    };
