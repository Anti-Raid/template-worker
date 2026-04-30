import { MAuthSyscall, MAuthSyscallRet } from './auth'
import { MBotSyscall, MBotSyscallRet } from './bot'
import { MDiscordSyscall, MDiscordSyscallRet } from './discord'
import { MGkvSyscall, MGkvSyscallRet } from './gkv'

/**
 * All possible top-level msyscall operation types
 */
export type MSyscallOp = MSyscallArgs['op'];

/**
 * Outer wrapper for all msyscall arguments
 */
export type MSyscallArgs = 
  | { 
      /** Authentication specific system calls */
      op: "Auth"; 
      /** The authentication request payload */
      req: MAuthSyscall 
    }
  | { 
      /** Bot specific system calls */
      op: "Bot"; 
      /** The bot request payload */
      req: MBotSyscall 
    }
  | { 
      /** Discord specific system calls */
      op: "Discord"; 
      /** The discord request payload */
      req: MDiscordSyscall 
    }
  | { 
      /** Global key-value specific system calls */
      op: "Gkv"; 
      /** The global key-value request payload */
      req: MGkvSyscall 
    };

/**
 * Outer wrapper for all successful msyscall returns
 */
export type MSyscallRet = 
  | { 
      /** Authentication specific system call response */
      op: "Auth"; 
      /** The authentication response data */
      data: MAuthSyscallRet 
    }
  | { 
      /** Bot specific system call response */
      op: "Bot"; 
      /** The bot response data */
      data: MBotSyscallRet 
    }
  | { 
      /** Discord specific system call response */
      op: "Discord"; 
      /** The discord response data */
      data: MDiscordSyscallRet 
    }
  | { 
      /** Global key-value specific system call response */
      op: "Gkv"; 
      /** The global key-value response data */
      data: MGkvSyscallRet 
    };

/**
 * Outer wrapper for all msyscall errors
 */
export type MSyscallError = 
  | { 
      /** A generic error response */
      op: "Generic"; 
      /** The error message */
      message: string 
    }
  | { 
      /** Invalid event name or data was provided */
      op: "InvalidEvent"; 
      /** The reason why the event was invalid */
      reason: string 
    }
  | { 
      /** The current API context is too insecure to perform this operation (admin only) */
      op: "ContextInsecure" 
    }
  | { 
      /** This operation requires a user context but none was found */
      op: "ContextRequiresUser" 
    }
  | { 
      /** This operation requires an OAuth2 login token to work */
      op: "ContextRequiresOauth" 
    }
  | { 
      /** The bot is not present on the specified guild */
      op: "BotNotOnGuild" 
    }
  | { 
      /** The user needs to login via OAuth2 at least once before using this API */
      op: "UserOauth2Needed" 
    }
  | { 
      /** An authentication specific error occurred */
      op: "AuthError"; 
      /** The specific authentication error reason */
      reason: any 
    }
  | { 
      /** The request is unauthorized */
      op: "Unauthorized"; 
      /** The reason why the request was unauthorized */
      reason: string 
    }
  | { 
      /** The requested entity was not found */
      op: "EntityNotFound"; 
      /** The reason or description of the missing entity */
      reason: string 
    };
