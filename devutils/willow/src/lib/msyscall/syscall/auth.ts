import { type UserSession } from '../types/auth'
import { type PartialUser } from '../types/discord'


export type MAuthSyscall = {
    /** Creates a login session using oauth2 */
    op: "CreateLoginSession",
    /** The Discord OAuth2 code */
    code: string,
    /** The redirect URI used for the OAuth2 flow */
    redirect_uri: string,
    /** Optional code verifier for PKCE (App only) */
    code_verifier?: string | null
} | {
    /** Creates an API token */
    op: "CreateApiSession",
    /** The name of the API session/token */
    name: string,
    /** Expiry time in seconds from now */
    expiry: number
} | {
    /** Gets the current user's sessions */
    op: "GetUserSessions"
} | {
    /** Delete a session (login or api) */
    op: "DeleteSession",
    /** The ID of the session to delete */
    session_id: string
}

export type MAuthSyscallRet = {
    /** A created session returned by a syscall */
    op: "CreatedSession",
    /** Session metadata */
    session: UserSession,
    /** The session token (only returned upon creation) */
    token: string,
    /** The user who created the session (only sent on OAuth2 login) */
    user?: PartialUser | null
} | {
    /** List of user sessions */
    op: "UserSessions",
    /** The list of active sessions for the user */
    sessions: UserSession[]
} | {
    /** Ack/success response */
    op: "Ack"
}