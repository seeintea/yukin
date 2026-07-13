import type { ProviderInput, ProviderOutput } from "#/domain/provider";
import { get, save } from "#/utils/storage";
import { nanoid } from "nanoid";

const dbName = "yukin.provider";

export async function getProviders(): Promise<ProviderOutput[]> {
  return get(dbName);
}

export async function saveProvider(provider: ProviderInput): Promise<boolean> {
  let ret = false;
  try {
    const val = await get<ProviderOutput>(dbName);
    const saveItem = {
      id: nanoid(),
      ...provider,
    };
    val.push(saveItem);
    await save(dbName, val);
    ret = true;
  } catch {
    ret = false;
  }
  return ret;
}
