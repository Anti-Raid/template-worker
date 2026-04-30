export type ObjectStorageCall = 
  | { op: "ListFileMetas"; prefix?: string | null }
  | { op: "GetFileMeta"; key: string }
  | { op: "GetFileUrl"; key: string; expiry: number } // expiry in seconds
  | { op: "DownloadFile"; key: string }
  | { op: "UploadFile"; key: string; data: number[] } // Byte array
  | { op: "DeleteFile"; key: string };

export interface ObjectMetadata {
  key: string;
  last_modified?: string | null;
  size: number;
  etag?: string | null;
}

export type ObjectStorageResult = 
  | { op: "ObjectMetadata"; objs: ObjectMetadata[] }
  | { op: "FileUrl"; url: string }
  | { op: "Blob"; data: number[] }
  | { op: "Ack" };
