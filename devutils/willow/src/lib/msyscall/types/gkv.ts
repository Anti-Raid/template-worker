import type { KhronosValue } from '../khronosvalue'

export interface PartialGlobalKv {
  key: string;
  version: number;
  owner_id: string;
  owner_type: string;
  price?: number | null;
  short: string;
  public_metadata: KhronosValue;
  scope: string;
  created_at: string;
  last_updated_at: string;
  public_data: boolean;
  review_state: string;
  long?: string | null;
  data?: KhronosValue | null;
}
