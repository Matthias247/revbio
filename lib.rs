// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[crate_id = "revbio#0.1"];

#[comment = "Rust evented based I/O"];
#[license = "MIT/ASL2"];
#[crate_type = "lib"];

#[no_uv];
extern mod native;
extern mod collections;

use std::io::IoError;

pub use eventqueue::EventQueue;

pub mod events;
mod eventqueue;

#[cfg(target_os = "linux")]
#[cfg(target_os = "android")]
#[path="linux/eventqueueimpl.rs"]
mod eventqueueimpl;

#[cfg(target_os = "linux")]
#[cfg(target_os = "android")]
#[path="linux/timer.rs"]
pub mod timer;

#[cfg(target_os = "linux")]
#[cfg(target_os = "android")]
#[path="linux/syscalls.rs"]
#[allow(dead_code)]
mod syscalls;

#[cfg(target_os = "linux")]
#[cfg(target_os = "android")]
#[path="linux/helpers.rs"]
mod helpers;

#[cfg(target_os = "linux")]
#[cfg(target_os = "android")]
#[path="linux/tcp.rs"]
pub mod tcp;

#[cfg(target_os = "linux")]
#[cfg(target_os = "android")]
#[path="linux/channel.rs"]
pub mod channel;

/// Holds either the success value of an IO operation or an error
pub type IoResult<T> = Result<T, IoError>;