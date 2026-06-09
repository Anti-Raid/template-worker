import { type KhronosValue } from '../khronosvalue'

export type StateOp = 
  | { op: "KvFind"; query: string; scope: string }
  | { op: "KvGet"; key: string; scope: string }
  | { op: "KvGetWithBlob"; key: string; scope: string }
  | { op: "KvSet"; key: string; scope: string; value: KhronosValue, blob?: number[] }
  | { op: "KvDelete"; key: string; scope: string }
  | { op: "GlobalKvFind"; query: string; scope: string }
  | { op: "GlobalKvGet"; key: string; version: number; scope: string }
  | { op: "GlobalKvCreate"; key: string; version: number; short: string; public_metadata: KhronosValue; scope: string; public_data: boolean; long?: string | null; data: KhronosValue }
  | { op: "GlobalKvDelete"; key: string; version: number; scope: string }
  | { op: "GlobalKvGetData"; key: string; version: number; scope: string }
  | { op: "SubscribeEvent"; event: string; system: string }
  | { op: "UnsubscribeEvent"; event: string; system: string };

export interface KvLookup {
  key: string;
  value: KhronosValue;
  scope: string;
  created_at: string;
  last_updated_at: string;
}

export interface GlobalKv {
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
}

export type StateExecResult = 
  | { op: "Kv"; l: KvLookup }
  | { op: "KvWithBlob"; l: KvLookup; blob?: number[] }
  | { op: "GlobalKv"; l: GlobalKv }
  | { op: "GlobalKvData"; data: KhronosValue }
  | { op: "GlobalKvDataOpaque"; data: KhronosValue };

export interface TenantState {
  events: Record<string, string[]>;
  modflags: number;
}

export interface StateExecResponse {
  results: StateExecResult[];
  new_tenant_state?: TenantState | null;
}
