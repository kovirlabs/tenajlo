//! Moving files to the OS trash (CLAUDE.md rule 10: discard is recoverable).

use std::path::Path;

/// Moves `path` to the OS trash / recycle bin.
pub fn move_to_trash(path: &Path) -> Result<(), String> {
    #[allow(unused_mut)]
    let mut ctx = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // The Finder method shells out to osascript and triggers an Automation permission prompt.
        ctx.set_delete_method(DeleteMethod::NsFileManager);
    }
    ctx.delete(path).map_err(|e| e.to_string())
}
