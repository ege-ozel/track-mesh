//! Bounded HTTPS transport for encrypted announcements. Core owns project decisions.
mod http;
pub use http::{validate_mailbox_url, MailboxClient};
