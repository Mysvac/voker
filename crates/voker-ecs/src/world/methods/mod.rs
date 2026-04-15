//! High-level [`World`] operations.
//!
//! This module groups method implementations by concern so the `World` API can
//! remain discoverable without putting all methods in one file.
//!
//! Notable groups include:
//! - spawn/despawn and entity lifecycle,
//! - component/resource registration and access,
//! - query and schedule integration,
//! - deferred command and message utilities.

mod arche;
mod command;
mod components;
mod despawn;
mod event;
mod forget;
mod hook;
mod message;
mod modify;
mod observer;
mod query;
mod register;
mod resource;
mod schedule;
mod spawn;
mod system;
mod uninit;
