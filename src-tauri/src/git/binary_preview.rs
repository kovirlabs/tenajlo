//! Sizes and image previews for binary and Git LFS files (spec §8.1, v1.4), shown instead of
//! "this binary file has changed". Reads blobs with `git cat-file` and the working file from
//! disk; LFS content comes only from the local LFS store, never the network.

use std::path::{Component, Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::lfs_pointer::{parse_pointer, MAX_POINTER_BYTES};

/// Images up to this size are previewed (spec: 10 MB).
pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// Which file version pair to describe.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum PreviewSource {
    /// HEAD (old) against the working tree (new).
    Working {
        path: String,
        old_path: Option<String>,
    },
    /// A commit's first parent (old) against the commit (new).
    Commit {
        sha: String,
        path: String,
        old_path: Option<String>,
    },
}

/// One version of the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileVersion {
    /// Size of the real file (for LFS, from the pointer). A `number` in TypeScript.
    #[specta(type = u32)]
    pub size: u64,
    /// Stored in Git LFS.
    pub lfs: bool,
    /// A `data:` URL when this is a PNG, JPEG, GIF, WebP or BMP image within the size limit
    /// whose content is on this computer.
    pub image: Option<String>,
}

/// Both versions; `None` where the file doesn't exist (added or deleted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BinaryPreview {
    pub old: Option<FileVersion>,
    pub new: Option<FileVersion>,
}

/// Describes the two versions of a binary or LFS file. `sha` must already be validated.
pub async fn preview(
    git: &GitBinary,
    root: &Path,
    source: &PreviewSource,
) -> Result<BinaryPreview, GitError> {
    let lfs_dir = lfs_objects_dir(git, root).await?;
    let (old_rev, new) = match source {
        PreviewSource::Working { path, old_path } => {
            check_path(path)?;
            let old = old_path.as_deref().unwrap_or(path);
            check_path(old)?;
            let new = working_version(&root.join(path), &lfs_dir).await;
            (format!("HEAD:{old}"), new)
        }
        PreviewSource::Commit {
            sha,
            path,
            old_path,
        } => {
            check_path(path)?;
            let old = old_path.as_deref().unwrap_or(path);
            check_path(old)?;
            let new = blob_version(git, root, &format!("{sha}:{path}"), &lfs_dir).await?;
            (format!("{sha}^:{old}"), new)
        }
    };
    let old = blob_version(git, root, &old_rev, &lfs_dir).await?;
    Ok(BinaryPreview { old, new })
}

/// Repository-relative path without `..`, and no newline (it goes on a `cat-file` line).
fn check_path(path: &str) -> Result<(), GitError> {
    let ok = !path.is_empty()
        && !path.contains('\n')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
    ok.then_some(())
        .ok_or_else(|| GitError::Parse(format!("not a repository path: {path}")))
}

/// `<git common dir>/lfs/objects`, where git-lfs keeps downloaded content.
async fn lfs_objects_dir(git: &GitBinary, root: &Path) -> Result<PathBuf, GitError> {
    let out = GitCommand::new(["rev-parse", "--git-common-dir"], Access::ReadOnly)
        .cwd(root)
        .run(git)
        .await?;
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    Ok(root.join(dir).join("lfs").join("objects"))
}

/// The version at `rev` (`<commit>:<path>`), or `None` if it doesn't exist there.
async fn blob_version(
    git: &GitBinary,
    root: &Path,
    rev: &str,
    lfs_dir: &Path,
) -> Result<Option<FileVersion>, GitError> {
    // --batch-check reads the name from stdin: no option parsing, any path, and "missing"
    // instead of an error for an added or deleted file (or a root commit's parent).
    let out = GitCommand::new(["cat-file", "--batch-check"], Access::ReadOnly)
        .cwd(root)
        .stdin(format!("{rev}\n"))
        .run(git)
        .await?;
    let line = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let mut fields = line.split(' ');
    let (Some(oid), Some("blob"), Some(size)) = (fields.next(), fields.next(), fields.next())
    else {
        return Ok(None);
    };
    let size: u64 = size.parse().map_err(|_| GitError::Parse(line.clone()))?;
    if !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(GitError::Parse(line));
    }
    if size > MAX_IMAGE_BYTES {
        return Ok(Some(FileVersion {
            size,
            lfs: false,
            image: None,
        }));
    }
    let bytes = GitCommand::new(["cat-file", "blob", oid], Access::ReadOnly)
        .cwd(root)
        .run(git)
        .await?
        .stdout;
    Ok(Some(from_bytes(bytes, lfs_dir).await))
}

/// The file on disk, or `None` if it's gone (deleted) or not a regular file.
async fn working_version(path: &Path, lfs_dir: &Path) -> Option<FileVersion> {
    let meta = tokio::fs::symlink_metadata(path).await.ok()?;
    if !meta.is_file() {
        return None;
    }
    if meta.len() > MAX_IMAGE_BYTES {
        return Some(FileVersion {
            size: meta.len(),
            lfs: false,
            image: None,
        });
    }
    let bytes = tokio::fs::read(path).await.ok()?;
    Some(from_bytes(bytes, lfs_dir).await)
}

/// Describes `bytes`; an LFS pointer is followed to the local LFS store when it's there.
async fn from_bytes(bytes: Vec<u8>, lfs_dir: &Path) -> FileVersion {
    let pointer = (bytes.len() <= MAX_POINTER_BYTES)
        .then(|| std::str::from_utf8(&bytes).ok().and_then(parse_pointer))
        .flatten();
    let Some(pointer) = pointer else {
        return FileVersion {
            size: bytes.len() as u64,
            lfs: false,
            image: image_data_url(&bytes),
        };
    };
    let image = if pointer.size <= MAX_IMAGE_BYTES {
        let object = lfs_dir
            .join(&pointer.oid[0..2])
            .join(&pointer.oid[2..4])
            .join(&pointer.oid);
        tokio::fs::read(object)
            .await
            .ok()
            .and_then(|b| image_data_url(&b))
    } else {
        None
    };
    FileVersion {
        size: pointer.size,
        lfs: true,
        image,
    }
}

/// The image type from its first bytes (never from the file name).
fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") && bytes.len() > 26 {
        Some("image/bmp")
    } else {
        None
    }
}

fn image_data_url(bytes: &[u8]) -> Option<String> {
    let mime = image_mime(bytes)?;
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{data}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::{commit_all, git, git_in, init_repo};

    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, 0x49, 0x48, 0x44, 0x52, 0,
        0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1F, 0x15, 0xC4, 0x89, 0, 0, 0, 0x0A, 0x49, 0x44,
        0x41, 0x54, 0x78, 0x9C, 0x63, 0, 1, 0, 0, 5, 0, 1, 0x0D, 0x0A, 0x2D, 0xB4, 0, 0, 0, 0,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn sniffs_images_by_content() {
        assert_eq!(image_mime(PNG_1X1), Some("image/png"));
        assert_eq!(image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(image_mime(b"GIF89a...."), Some("image/gif"));
        assert_eq!(image_mime(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_mime(b"<svg xmlns=..."), None, "SVG is never inlined");
        assert_eq!(image_mime(b"BM"), None);
        assert!(image_data_url(PNG_1X1)
            .unwrap()
            .starts_with("data:image/png;base64,iVBORw0KGgo"));
    }

    #[test]
    fn rejects_paths_outside_the_repository() {
        for bad in ["", "../x", "/etc/passwd", "a\nb", "./a"] {
            assert!(check_path(bad).is_err(), "{bad:?}");
        }
        assert!(check_path("img/Pumpe Gehäuse.png").is_ok());
    }

    #[tokio::test]
    async fn working_and_commit_versions_of_an_image() {
        let (_dir, root) = init_repo().await;
        let root = root.as_path();
        let g = git();
        std::fs::create_dir(root.join("img")).unwrap();
        std::fs::write(root.join("img/logo ä.png"), PNG_1X1).unwrap();
        commit_all(root, "logo").await;
        let sha = git_in(root, &["rev-parse", "HEAD"]).await.trim().to_owned();

        // Root commit: no old version.
        let p = preview(
            &g,
            root,
            &PreviewSource::Commit {
                sha: sha.clone(),
                path: "img/logo ä.png".into(),
                old_path: None,
            },
        )
        .await
        .unwrap();
        assert!(p.old.is_none());
        let new = p.new.unwrap();
        assert_eq!((new.size, new.lfs), (PNG_1X1.len() as u64, false));
        assert!(new.image.unwrap().starts_with("data:image/png"));

        // Working tree: the file grew and is no longer an image.
        std::fs::write(root.join("img/logo ä.png"), b"not an image any more").unwrap();
        let p = preview(
            &g,
            root,
            &PreviewSource::Working {
                path: "img/logo ä.png".into(),
                old_path: None,
            },
        )
        .await
        .unwrap();
        assert!(p.old.unwrap().image.is_some());
        let new = p.new.unwrap();
        assert_eq!((new.size, new.image), (21, None));

        // Deleted: no new version.
        std::fs::remove_file(root.join("img/logo ä.png")).unwrap();
        let src = PreviewSource::Working {
            path: "img/logo ä.png".into(),
            old_path: None,
        };
        assert!(preview(&g, root, &src).await.unwrap().new.is_none());
    }

    /// The real path: Tenajlo's per-invocation LFS filters, a working-tree diff that comes
    /// back as `Lfs`, and a preview whose old image is read from the local LFS store.
    #[tokio::test]
    async fn lfs_image_diff_and_preview_end_to_end() {
        let (_dir, root) = init_repo().await;
        let root = root.as_path();
        let lfs_git = git().with_lfs(true);
        let run = |args: &'static [&'static str]| {
            let lfs_git = lfs_git.clone();
            async move {
                let mut full = vec!["-c", "user.name=T", "-c", "user.email=t@x"];
                full.extend_from_slice(args);
                GitCommand::new(full, Access::Mutating)
                    .cwd(root)
                    .run(&lfs_git)
                    .await
                    .unwrap();
            }
        };
        std::fs::write(
            root.join(".gitattributes"),
            "*.png filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        std::fs::write(root.join("logo.png"), PNG_1X1).unwrap();
        run(&["add", "-A"]).await;
        run(&["commit", "-q", "-m", "logo"]).await;
        std::fs::write(root.join("logo.png"), b"GIF89a and then some").unwrap();

        let current = crate::git::status::status(&lfs_git, root).await.unwrap();
        let file = current.files.iter().find(|f| f.path == "logo.png").unwrap();
        let diff = crate::git::diff::working_dir_diff(&lfs_git, root, file, true)
            .await
            .unwrap();
        let crate::git::parse::diff::FileDiff::Lfs { old, new } = diff else {
            panic!("expected an LFS diff, got {diff:?}");
        };
        assert_eq!(old.unwrap().size, PNG_1X1.len() as u64);
        assert_eq!(new.unwrap().size, 20);

        let src = PreviewSource::Working {
            path: "logo.png".into(),
            old_path: None,
        };
        let p = preview(&lfs_git, root, &src).await.unwrap();
        let old = p.old.unwrap();
        assert!(old.lfs && old.image.unwrap().starts_with("data:image/png"));
        let new = p.new.unwrap();
        assert_eq!((new.size, new.lfs), (20, false));
        assert!(new.image.unwrap().starts_with("data:image/gif"));
    }

    #[tokio::test]
    async fn lfs_pointers_report_the_real_size_and_use_the_local_store() {
        let dir = tempfile::tempdir().unwrap();
        let lfs_dir = dir.path().join("lfs/objects");
        let oid = "c52f0d630543e6f5eda8432f240153a68fa2ed86b89c1436035d57478e74ac5e";
        let pointer = format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {}\n",
            PNG_1X1.len()
        );
        let v = from_bytes(pointer.clone().into_bytes(), &lfs_dir).await;
        assert_eq!(
            (v.size, v.lfs, v.image.is_some()),
            (PNG_1X1.len() as u64, true, false)
        );

        let object_dir = lfs_dir.join(&oid[0..2]).join(&oid[2..4]);
        std::fs::create_dir_all(&object_dir).unwrap();
        std::fs::write(object_dir.join(oid), PNG_1X1).unwrap();
        let v = from_bytes(pointer.into_bytes(), &lfs_dir).await;
        assert!(v.lfs && v.image.unwrap().starts_with("data:image/png"));
    }
}
