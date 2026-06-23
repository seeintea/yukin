#![allow(dead_code)]

use path_clean::PathClean;

use crate::{AppError, AppResult};
use std::path::{Path, PathBuf};

/// 把用户提供的相对/绝对路径解析为 `root` 内的绝对路径,拒绝穿越。
///
/// 行为:
/// - `user` 是相对路径 → `root.join(user)` 再 clean(消除字面 `..`)
/// - `user` 是绝对路径 → 直接 clean(若不在 `root` 下会被最终的 `starts_with` 拒绝)
/// - canonicalize 解符号链接;真值发生在文件系统层面,无法绕过
/// - 目标文件不存在(写入场景)→ canonicalize 到最近存在的祖先,再拼剩余路径
pub fn resolve_within(root: &Path, user: &str) -> AppResult<PathBuf> {
    let user_path = Path::new(user);
    let joined = if user_path.is_absolute() {
        user_path.to_path_buf()
    } else {
        root.join(user_path)
    }
    .clean();

    let root_canon = root
        .canonicalize()
        .map_err(|_| AppError::Other("workspace root not found".into()))?;

    // 文件不存在(写入场景):向上找最近存在的祖先 canonicalize,再把剩余 tail 拼回去。
    // 这样符号链接在 ancestor 层面被解开,后续 starts_with 比较才可靠。
    let candidate_canon = match joined.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            let mut ancestor = joined.as_path();
            let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
            loop {
                match ancestor.canonicalize() {
                    Ok(real) => {
                        let mut p = real;
                        for seg in tail.iter().rev() {
                            p.push(seg);
                        }
                        break p;
                    }
                    Err(_) => {
                        let name = ancestor
                            .file_name()
                            .ok_or_else(|| AppError::PathEscape(user.into()))?;
                        tail.push(name);
                        ancestor = ancestor
                            .parent()
                            .ok_or_else(|| AppError::PathEscape(user.into()))?;
                    }
                }
            }
        }
    };

    if !candidate_canon.starts_with(&root_canon) {
        return Err(AppError::PathEscape(user.into()));
    }
    Ok(candidate_canon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    /// 工具函数:建一个临时 workspace 根目录(已 canonicalize)。
    fn make_root() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("create tempdir");
        let root = dir.path().canonicalize().expect("canonicalize tempdir");
        (dir, root)
    }

    #[test]
    fn relative_inside_existing_file_ok() {
        let (_dir, root) = make_root();
        fs::write(root.join("hello.txt"), "hi").unwrap();

        let p = resolve_within(&root, "hello.txt").unwrap();
        assert_eq!(p, root.join("hello.txt"));
    }

    #[test]
    fn relative_inside_nested_dir_ok() {
        let (_dir, root) = make_root();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub/a.txt"), "x").unwrap();

        let p = resolve_within(&root, "sub/a.txt").unwrap();
        assert_eq!(p, root.join("sub/a.txt"));
    }

    #[test]
    fn nonexistent_file_in_existing_dir_ok() {
        // 写入场景:文件不存在,但父目录存在
        let (_dir, root) = make_root();
        let p = resolve_within(&root, "new.txt").unwrap();
        assert_eq!(p, root.join("new.txt"));
    }

    #[test]
    fn nonexistent_nested_in_existing_dir_ok() {
        // 写入场景:多层不存在,但更上层存在
        let (_dir, root) = make_root();
        fs::create_dir(root.join("sub")).unwrap();
        let p = resolve_within(&root, "sub/notyet/deeper/file.txt").unwrap();
        assert_eq!(p, root.join("sub/notyet/deeper/file.txt"));
    }

    #[test]
    fn dotdot_traversal_rejected() {
        let (_dir, root) = make_root();
        let err = resolve_within(&root, "../etc/passwd").unwrap_err();
        assert!(matches!(err, AppError::PathEscape(_)));
    }

    #[test]
    fn double_dotdot_traversal_rejected() {
        let (_dir, root) = make_root();
        let err = resolve_within(&root, "../../etc/passwd").unwrap_err();
        assert!(matches!(err, AppError::PathEscape(_)));
    }

    #[test]
    fn absolute_path_outside_rejected() {
        let (_dir, root) = make_root();
        let err = resolve_within(&root, "/etc/passwd").unwrap_err();
        assert!(matches!(err, AppError::PathEscape(_)));
    }

    #[test]
    fn absolute_path_inside_ok() {
        let (_dir, root) = make_root();
        fs::write(root.join("hello.txt"), "hi").unwrap();
        // 用绝对路径但指向 root 内部
        let inside = root.join("hello.txt");
        let p = resolve_within(&root, inside.to_str().unwrap()).unwrap();
        assert_eq!(p, inside);
    }

    #[test]
    fn symlink_escaping_root_rejected() {
        // 防穿越的核心案例:在 root 内放一个符号链接,目标在 root 外
        let (_dir, root) = make_root();
        let outside_dir = TempDir::new().unwrap();
        let outside_path = outside_dir.path().canonicalize().unwrap();
        fs::write(outside_path.join("secret"), "x").unwrap();

        symlink(&outside_path, root.join("escape")).unwrap();

        let err = resolve_within(&root, "escape/secret").unwrap_err();
        assert!(matches!(err, AppError::PathEscape(_)));
    }

    #[test]
    fn symlink_inside_root_ok() {
        // 内部 symlink 指向 root 内的另一处,应允许
        let (_dir, root) = make_root();
        fs::create_dir(root.join("real")).unwrap();
        fs::write(root.join("real/data.txt"), "x").unwrap();
        symlink(root.join("real"), root.join("alias")).unwrap();

        let p = resolve_within(&root, "alias/data.txt").unwrap();
        // canonicalize 解开 symlink → 应该指向 real/data.txt
        assert_eq!(p, root.join("real/data.txt"));
    }

    #[test]
    fn dot_segments_collapsed() {
        let (_dir, root) = make_root();
        fs::write(root.join("hello.txt"), "hi").unwrap();
        let p = resolve_within(&root, "./sub/../hello.txt").unwrap();
        assert_eq!(p, root.join("hello.txt"));
    }

    #[test]
    fn dotdot_at_end_after_inside_dir_still_inside() {
        // sub/.. 应该 clean 回 root,合法
        let (_dir, root) = make_root();
        fs::create_dir(root.join("sub")).unwrap();
        let p = resolve_within(&root, "sub/..").unwrap();
        assert_eq!(p, root);
    }

    #[test]
    fn empty_string_resolves_to_root() {
        let (_dir, root) = make_root();
        let p = resolve_within(&root, "").unwrap();
        assert_eq!(p, root);
    }
}
