import { RawKhronosValue } from '../khronosvalue'
import { BotStatus, BotConfig } from '../types/bot'
import { Id } from '../types/common'
import { StateOp, StateExecResult, TenantState } from '../types/state'
import { ObjectStorageCall, ObjectStorageResult } from '../types/objstore'

export type MBotSyscall = 
  | { 
      /** Returns the commands registered on the bot */
      op: "GetBotCommands" 
    }
  | { 
      /** Returns the bots base config */
      op: "GetBotConfig" 
    }
  | { 
      /** Returns the bots status */
      op: "GetBotStatus" 
    }
  | { 
      /** Dispatch an event to a worker process */
      op: "DispatchEvent"; 
      /** Tenant ID to dispatch the event to */
      id: Id; 
      /** Name of the event. Must start with 'Web' for non-admins */
      name: string; 
      /** Data to send along with the event */
      data: RawKhronosValue 
    }
  | { 
      /** Dispatch an event to a worker process with some safety checks removed (Secure only) */
      op: "AdminRelaxedDispatchEvent"; 
      /** Tenant ID to dispatch the event to */
      id: Id; 
      /** Name of the event */
      name: string; 
      /** Data to send */
      data: RawKhronosValue; 
      /** Whether or not to allow non-Web event names */
      allow_non_web_event_names: boolean; 
      /** Whether or not to allow sending events to yourself */
      allow_self_event: boolean; 
      /** The author ID to mock */
      mock_id?: string | null 
    }
  | { 
      /** Returns the uncached bot status (Secure only) */
      op: "AdminGetUncachedBotStatus" 
    }
  | { 
      /** Admin API to drop a tenant (Secure only) */
      op: "AdminDropTenant"; 
      /** The ID of the tenant to drop */
      id: Id 
    }
  | { 
      /** Admin API to set tenant state moderation flags (Secure only) */
      op: "AdminSetTenantStateModFlags"; 
      /** The ID of the tenant */
      id: Id; 
      /** The new moderation flags bitfield */
      modflags: number 
    }
  | { 
      /** Admin API to run a set of state ops on a tenant (Secure only) */
      op: "AdminState"; 
      /** The ID of the tenant */
      id: Id; 
      /** The list of state operations to perform */
      ops: StateOp[] 
    }
  | { 
      /** Admin API to run an object storage op on a tenant (Secure only) */
      op: "AdminObjectStorage"; 
      /** The ID of the tenant */
      id: Id; 
      /** The object storage call details */
      call: ObjectStorageCall 
    }
  | { 
      /** Admin API to fetch tenant state for a tenant (Secure only) */
      op: "AdminFetchTenantState"; 
      /** The ID of the tenant */
      id: Id 
    };

export type MBotSyscallRet = 
  | { 
      /** A list of bot commands */
      op: "CommandList"; 
      /** The list of commands registered on the bot */
      commands: any[] 
    }
  | { 
      /** Bot configuration response */
      op: "BotConfig"; 
      /** The ID of the main support server */
      main_server: string; 
      /** Discord support server invite link */
      support_server_invite: string; 
      /** The Discord client ID of the bot */
      client_id: string 
    }
  | { 
      /** Bot status response */
      op: "BotStatus"; 
      /** Current status information of the bot and its shards */
      status: BotStatus 
    }
  | { 
      /** Response containing a Khronos value */
      op: "KhronosValue"; 
      /** The returned data */
      data: RawKhronosValue 
    }
  | { 
      /** State execution results (Admin only) */
      op: "State"; 
      /** The results of the performed state operations */
      res: StateExecResult[]; 
      /** The updated tenant state, if it was changed */
      new_tenant_state?: TenantState | null 
    }
  | { 
      /** Object storage operation result (Admin only) */
      op: "ObjectStorage"; 
      /** The result of the object storage operation */
      res: ObjectStorageResult 
    }
  | { 
      /** Tenant state response (Admin only) */
      op: "TenantState"; 
      /** The requested tenant state */
      ts: TenantState 
    }
  | { 
      /** Generic success acknowledgement */
      op: "Ack" 
    };
