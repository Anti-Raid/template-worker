import { encode, decode } from '../khronosvalue'
import { UserSession } from '../types/auth'
import { PartialUser } from '../types/discord'

export type MAuthSyscall = {
    // Creates a login session using oauth2
    op: "CreateLoginSession",
    code: string,
    redirect_uri: string,
    code_verifier?: string | null // app only
} | {
    // Creates an API token 
    op: "CreateApiSession",
    name: string,
    expiry: number // expiry in seconds
} | {
    // Gets user sessions
    op: "GetUserSessions"
} | {
    // Delete a session (login or api)
    op: "DeleteSession",
    session_id: string
}

export type MAuthSyscallRet = {
    // A created session returned by a syscall
    op: "CreatedSession",
    // Session metadata
    session: UserSession,
    // Session token
    token: string,
    // The user who created the session (only sent on OAuth2 login)
    user?: PartialUser | null
} | {
    // List of user sessions
    op: "UserSessions",
    sessions: UserSession[]
} | {
    // Ack/success response
    op: "Ack"
}