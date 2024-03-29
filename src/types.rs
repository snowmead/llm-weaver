use std::fmt::Display;

use async_openai::types::Role;
use serde::{Deserialize, Serialize};

use crate::{Config, Llm};

pub type PromptModelTokens<T> = <<T as Config>::PromptModel as Llm<T>>::Tokens;
pub type SummaryModelTokens<T> = <<T as Config>::SummaryModel as Llm<T>>::Tokens;
pub type PromptModelRequest<T> = <<T as Config>::PromptModel as Llm<T>>::Request;

/// Base type for all configuration parameters.
pub type F32 = f32;

pub const SYSTEM_ROLE: &str = "system";
pub const ASSISTANT_ROLE: &str = "assistant";
pub const USER_ROLE: &str = "user";
pub const FUNCTION_ROLE: &str = "function";

/// Wrapped [`Role`] for custom implementations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WrapperRole {
	Role(Role),
}

impl Default for WrapperRole {
	fn default() -> Self {
		Self::Role(Role::User)
	}
}

impl From<&str> for WrapperRole {
	fn from(role: &str) -> Self {
		match role {
			SYSTEM_ROLE => Self::Role(Role::System),
			ASSISTANT_ROLE => Self::Role(Role::Assistant),
			USER_ROLE => Self::Role(Role::User),
			FUNCTION_ROLE => Self::Role(Role::Function),
			_ => panic!(
				"Invalid role: {} \n Valid roles: {} | {} | {} | {}",
				role, SYSTEM_ROLE, ASSISTANT_ROLE, USER_ROLE, FUNCTION_ROLE
			),
		}
	}
}

impl From<WrapperRole> for String {
	fn from(role: WrapperRole) -> Self {
		match role {
			WrapperRole::Role(Role::System) => SYSTEM_ROLE.to_string(),
			WrapperRole::Role(Role::Assistant) => ASSISTANT_ROLE.to_string(),
			WrapperRole::Role(Role::User) => USER_ROLE.to_string(),
			WrapperRole::Role(Role::Function) => FUNCTION_ROLE.to_string(),
			WrapperRole::Role(_) => panic!("Invalid role"),
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum LoomError {
	Weave(#[from] WeaveError),
	Storage(#[from] StorageError),
}

impl Display for LoomError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Weave(e) => write!(f, "{}", e),
			Self::Storage(e) => write!(f, "{}", e),
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum WeaveError {
	/// Bad configuration
	BadConfig(String),
}

impl Display for WeaveError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::BadConfig(msg) => write!(f, "Bad configuration: {}", msg),
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
	Redis(redis::RedisError),
	Parsing,
	NotFound,
}

impl Display for StorageError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			StorageError::Redis(e) => write!(f, "Redis error: {}", e),
			StorageError::Parsing => write!(f, "Parsing error"),
			StorageError::NotFound => write!(f, "Not found"),
		}
	}
}
