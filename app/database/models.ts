import { type EntityTable } from "dexie"

export interface ModelPlatform {
  id: number,
  name: string
}
export type ModelPlatformTable = EntityTable<ModelPlatform, 'id'>
export const modelPlatformSchema = '++id, name'

export interface APIKey {
  id: string,
  platform: number,
  uri: string,
  key: string
}
export type APIKeyTable = EntityTable<APIKey, 'id'>
export const apiKeySchema = '++id, platform, uri, key'