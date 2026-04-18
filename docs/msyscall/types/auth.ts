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