# Phase D — Workspace + 文件系统命令

> 创建日期: 2026-06-06
> 目标: dialog 选工作目录,`safe_join` 防穿越 + 6 个 fs 命令全实现。

## 前置
- Phase C 完成

## 步骤

1. **实现 `path_safety.rs`**:
   ```rust
   pub fn safe_join(root: &Path, user: &str) -> AppResult<PathBuf> {
       let user_path = Path::new(user);
       let joined = if user_path.is_absolute() {
           user_path.to_path_buf()
       } else {
           root.join(user_path)
       }.clean();   // path-clean: 字面解 ..

       let root_canon = root.canonicalize()
           .map_err(|_| AppError::Other("workspace root not found".into()))?;

       let candidate_canon = match joined.canonicalize() {
           Ok(p) => p,
           Err(_) => {  // 写入场景,文件可能未存在
               let parent = joined.parent()
                   .ok_or(AppError::PathEscape(user.into()))?
                   .canonicalize()?;
               parent.join(joined.file_name().unwrap())
           }
       };
       if !candidate_canon.starts_with(&root_canon) {
           return Err(AppError::PathEscape(user.into()));
       }
       Ok(candidate_canon)
   }
   ```
   + `#[cfg(test)] mod tests`:
   - `../../etc/passwd` → `PathEscape`
   - 绝对路径在 root 外 → `PathEscape`
   - 绝对路径在 root 内 → OK
   - 未存在文件(写入)→ OK
   - symlink 指向 root 外 → `PathEscape`(canonicalize 解符号链)
   - `.`, 空串 → 退化情况

2. **`commands/workspace.rs`**:
   ```rust
   #[tauri::command]
   pub async fn select_workspace(app: AppHandle, state: State<'_, AppState>) -> AppResult<String> {
       use tauri_plugin_dialog::DialogExt;
       let folder = app.dialog().file().blocking_pick_folder()
           .ok_or(AppError::DialogCancelled)?;
       let path = folder.into_path()?.canonicalize()?;
       sqlx::query("INSERT INTO settings (key,value) VALUES ('workspace_path',?1)
                    ON CONFLICT(key) DO UPDATE SET value=?1")
           .bind(path.to_string_lossy().to_string()).execute(&state.db).await?;
       *state.workspace.write().await = Some(path.clone());
       Ok(path.to_string_lossy().into())
   }
   #[tauri::command]
   pub async fn get_workspace(state: State<'_, AppState>) -> AppResult<Option<String>> {
       Ok(state.workspace.read().await.as_ref().map(|p| p.to_string_lossy().into()))
   }
   // set_workspace 类似(无 dialog,带 path 参数)
   ```

3. **`commands/fs.rs`** — 6 个命令:
   ```rust
   const MAX_READ_BYTES: usize = 200_000;

   async fn workspace(state: &AppState) -> AppResult<PathBuf> {
       state.workspace.read().await.clone().ok_or(AppError::NoWorkspace)
   }

   #[tauri::command]
   pub async fn fs_read(path: String, state: State<'_, AppState>) -> AppResult<FsReadResult> {
       let root = workspace(&state).await?;
       let p = safe_join(&root, &path)?;
       let bytes = tokio::fs::read(&p).await?;
       let original_size = bytes.len();
       let (data, truncated) = if bytes.len() > MAX_READ_BYTES {
           (String::from_utf8_lossy(&bytes[..MAX_READ_BYTES]).to_string(), true)
       } else {
           (String::from_utf8_lossy(&bytes).to_string(), false)
       };
       Ok(FsReadResult { content: data, truncated, original_size })
   }

   // fs_write(path, content, create_dirs?)
   // fs_edit(path, search, replace, all?) → 返回 EditReport { replacements, before_excerpt, after_excerpt }
   // fs_list_dir(path) → Vec<DirEntry>
   // fs_glob(pattern) → Vec<String>  (相对 workspace 的路径)
   // fs_exists(path) → bool
   ```

4. **`WorkspaceSelector.tsx`** (`src/components/settings/`):
   - 显示当前 workspace(或 "未设置")
   - "选择文件夹" 按钮调 `tauri.workspace.select()`
   - 选定后回填 zustand store

## 关键文件
- `src-tauri/src/path_safety.rs`(实现 + 单元测试)
- `src-tauri/src/commands/workspace.rs`(实现)
- `src-tauri/src/commands/fs.rs`(实现 + 截断)
- `src/components/settings/WorkspaceSelector.tsx`(新)
- `src/lib/store/workspace.ts`(新 zustand store)

## 验证
- [ ] `cd src-tauri && cargo test path_safety` 全过
- [ ] 点 "选择文件夹",native dialog 弹出
- [ ] `invoke('fs_list_dir', {path:"."})` 返回入口数组
- [ ] `invoke('fs_read', {path:"package.json"})` 返回 JSON 文本
- [ ] `invoke('fs_write', {path:"plan/test.txt", content:"hello"})` 磁盘上出现
- [ ] `invoke('fs_edit', {path:"plan/test.txt", search:"hello", replace:"world"})` → replacements:1
- [ ] `invoke('fs_read', {path:"../../etc/passwd"})` → `{code:"path_escape"}` 错误
- [ ] `invoke('fs_glob', {pattern:"**/*.rs"})` 返回相对路径数组
- [ ] 写大文件(>200KB)然后 `fs_read` 看到 `truncated: true`

## 风险/陷阱
- `safe_join` 在 write 场景(parent 未存在的多层路径)要兜底:递归向上找最近存在 ancestor 再 canonicalize
- `fs_glob` 的 pattern 经 `safe_join` 不靠谱:直接 `glob(workspace.join(pattern))`,然后过滤每条结果 starts_with workspace
- Windows 路径分隔符:用 `Path::join`,不要手拼 `/`