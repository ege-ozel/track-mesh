//! Authenticated snapshot transfer, verified installation, and journal recovery.
//! Start with `Receiver`; disk mutation and recovery ordering live in `apply`.
mod apply;
mod assemble;
mod backups;
mod download;
mod installation;
mod journal;
mod receiver;
mod recovery;
mod serving;
mod verification;

pub use backups::{delete_backup, inspect_backup};
pub use download::DownloadApproval;
pub use journal::stage_path;
pub use receiver::{ReceiveRefusal, Receiver};
pub use recovery::recover_project;
pub use serving::serve;
pub use verification::validate_snapshot;
