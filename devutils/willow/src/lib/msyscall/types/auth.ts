export interface UserSession {
  /** The ID of the session */
  id: string;

  /** The name of the session */
  name?: string | null;

  /** The ID of the user who created the session */
  user_id: string;

  /** The time the session was created */
  created_at: string;

  /** The type of session (e.g., "login", "api") */
  type: string;

  /** The time the session expires */
  expiry: string;
}

/**
 * Represents an authorized session and its associated user
 */
export interface AuthorizedSession {
  /** User ID */
  user_id: string;
  /** Session ID */
  id: string;
  /** The state of the user */
  state: string;
  /** The type of session */
  type: string;
  }

  /**
  * Specific authentication errors that can occur during msyscalls
  */
  export type AuthError = 
  | { op: "InvalidRedirectUri" }
  | { op: "CodeTooShort" }
  | { op: "CodeReuseDetected" }
  | { op: "NeededScopesNotFound" }
  | { op: "ExpiryTimeOutOfRange" };