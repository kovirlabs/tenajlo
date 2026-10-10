//! The plain-language message shown for each [`GitErrorKind`]. Written for non-Git-experts.

use super::GitErrorKind;

impl GitErrorKind {
    /// Plain-language message for a classified git failure (spec §5.5).
    pub fn user_message(self) -> &'static str {
        match self {
            GitErrorKind::AuthFailed => "The server didn't accept your sign-in. Check your account or token.",
            GitErrorKind::HostKeyUnknown => {
                "The server's identity wasn't confirmed, so Tenajlo didn't connect. Try again and check the fingerprint with your IT team."
            }
            GitErrorKind::HostKeyChanged => {
                "The server's identity has changed since you last connected. This can mean someone is intercepting the connection. Don't continue: contact your IT team."
            }
            GitErrorKind::SshKeyRejected => {
                "The server didn't accept your SSH key. Add your public key (for example id_ed25519.pub in your .ssh folder) to your Forgejo account under Settings → SSH / GPG Keys, or clone with HTTPS instead."
            }
            GitErrorKind::PushRejected => "The server has changes you don't have yet. Pull first, then push.",
            GitErrorKind::PullDiverged => {
                "Your branch and the server's branch both have new commits. Merge the server's changes into your branch to continue."
            }
            GitErrorKind::LocalChangesBlock => "You have changes that would be overwritten.",
            GitErrorKind::TlsUntrusted => {
                "This computer doesn't trust the server's certificate. Your IT team may need to install the company certificate."
            }
            GitErrorKind::HostUnreachable => "Couldn't reach the server. Check your network or VPN connection.",
            GitErrorKind::MergeConflict => {
                "Some files were changed both on the server and by you. Resolve the conflicts, then commit to finish."
            }
            GitErrorKind::NotARepository => "This folder isn't a Git repository.",
            GitErrorKind::DubiousOwnership => {
                "Git won't open this folder because it belongs to a different user account. Ask IT to fix the folder's owner, or mark it as safe with `git config --global --add safe.directory <path>`."
            }
            GitErrorKind::IdentityMissing => "Git needs your name and email before you can commit.",
            GitErrorKind::BranchExists => "A branch with that name already exists.",
            GitErrorKind::BranchNotMerged => "This branch has commits that aren't on any other branch.",
            GitErrorKind::ProtectedBranch => {
                "The server doesn't allow pushing directly to this branch. Create a new branch for your changes, push that, and open a pull request on Forgejo."
            }
            GitErrorKind::PushDeclined => "The server refused this push. Its explanation is in the details.",
            GitErrorKind::PushTooLarge => {
                "The server refused the upload because it's too large. Large files like CAD models belong in Git LFS; ask the repository owner to set it up for those file types."
            }
            GitErrorKind::RemoteBranchMissing => {
                "This branch no longer exists on the server. Someone may have deleted it after it was merged."
            }
            GitErrorKind::RepositoryLocked => {
                "Another program seems to be using this repository. Close other Git tools and try again. If none are open, Git may have crashed earlier: delete the .lock file named in the details."
            }
            GitErrorKind::PathTooLong => {
                "A file path is too long for Windows. Clone the repository to a shorter folder (for example C:\\src), or ask IT to enable long path support."
            }
            GitErrorKind::DiskFull => "Your disk is full. Free up some space and try again.",
            GitErrorKind::TlsRevocationCheck => {
                "Windows couldn't check whether the server's certificate has been revoked, usually because the company's certificate list can't be reached from here. Connect to the company network or VPN and try again, or ask your IT team."
            }
            GitErrorKind::LfsFailed => {
                "Git LFS couldn't transfer some large files. Check your connection and try again; the details say which files."
            }
            GitErrorKind::RepositoryNotFound => {
                "The server couldn't find that repository. Check the address, and that your account has access to it."
            }
            GitErrorKind::Unknown => "Git reported a problem.",
        }
    }
}
