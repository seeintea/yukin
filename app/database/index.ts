import Dexie from 'dexie';

import { type ModelPlatformTable, type APIKeyTable, modelPlatformSchema, apiKeySchema } from './models'

const random = "WGg9qp"

const db = new Dexie(`${random}DB`) as Dexie & {
  model_platform: ModelPlatformTable,
  api_key: APIKeyTable
}

db.version(1).stores({
  model_platform: modelPlatformSchema,
  api_key: apiKeySchema
})

const init = async () => {
  const platCount = await db.model_platform.count()
  if (platCount === 0) {
    await db.model_platform.add({ name: 'DeepSeek' })
  }
}

export { db, init };
export { type ModelPlatform, type APIKey } from './models'