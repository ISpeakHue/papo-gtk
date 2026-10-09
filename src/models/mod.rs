//! Data models matching the Papo backend's OpenAPI schema.
//!
//! All fields are optional where the OpenAPI schema marks them `nullable: true`
//! or where the property is not in the `required` list.

pub mod auth;
pub mod channel;
pub mod message;
pub mod embed;
pub mod role;
pub mod server;
pub mod user;
pub mod permissions;
pub mod emoji;
pub mod discovery;

// Re-export common types at crate root for convenience.
pub use auth::*;
pub use channel::*;
pub use message::*;
pub use embed::*;
pub use role::*;
pub use server::*;
pub use user::*;
pub use permissions::*;
pub use emoji::*;
pub use discovery::*;

pub mod direct;
pub use direct::*;

pub mod moderation;
pub use moderation::*;

pub mod voice;
pub use voice::*;
