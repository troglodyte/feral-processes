//! Development tooling shared by the launcher's two bins.
//!
//! This crate is the game binary; the library target exists only so that
//! `feral-processes` and `savetool` can share `dev_template` — and so that
//! it can be unit-tested, which a module reached by `#[path]` from two bins
//! could not be without compiling its tests twice. Nothing about the game
//! itself lives here.

pub mod dev_template;
