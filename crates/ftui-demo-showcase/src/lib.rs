#![forbid(unsafe_code)]
#![allow(dead_code)]

//! FrankenTUI Demo Showcase library.
//!
//! This module exposes the demo application internals so that integration tests
//! can construct screens, render them, and assert snapshots.

pub mod app;
pub mod chrome;
pub mod cli;
pub mod data;
pub mod screens;
pub mod theme;
