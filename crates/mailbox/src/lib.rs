//! Bounded HTTPS transport for encrypted announcements. Core owns project decisions.
mod http;
pub use http::{validate_mailbox_url, validate_supabase_project_url, MailboxClient};
mod supabase;
pub use supabase::{SupabaseClient, SupabaseError};
