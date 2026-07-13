// 暂时使用 localStorage 实现本地持久化能力

// 以下约束由类型系统保证
// 1. 存储的 val 一定是 数组
// 2. 存储的 val 一定有 id 字段

export interface StorageItem {
  id: string;
}

export async function get<T extends StorageItem>(
  key: string,
  defaultVal: T[] = [],
): Promise<T[]> {
  const val = localStorage.getItem(key);
  if (val === null) return defaultVal;
  return JSON.parse(val) as T[];
}

export async function save<T extends StorageItem>(
  key: string,
  val: T[],
): Promise<void> {
  localStorage.setItem(key, JSON.stringify(val));
}

export async function del<T extends StorageItem>(
  key: string,
  id: string,
): Promise<T[]> {
  const val = await get<T>(key);

  const idx = val.findIndex((item) => item.id === id);
  if (idx > -1) {
    val.splice(idx, 1);
    await save(key, val);
  }

  return val;
}

export async function update<T extends StorageItem>(
  key: string,
  item: T,
): Promise<T[]> {
  const val = await get<T>(key);

  const idx = val.findIndex((current) => current.id === item.id);
  if (idx > -1) {
    val[idx] = item;
    await save(key, val);
  }

  return val;
}
