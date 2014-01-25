// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::libc;
use std::io;
use std::os;
use std::io::IoError;

#[cfg(unix)]
#[inline]
pub fn retry(f: || -> libc::c_int) -> libc::c_int {
	loop {
		match f() {
			-1 if os::errno() as int == libc::EINTR as int => {}
			n => return n,
		}
	}
}

pub fn translate_error(errno: i32, detail: bool) -> IoError {	

	#[cfg(not(windows))]
	fn get_err(errno: i32) -> (io::IoErrorKind, &'static str) {
		// XXX: this should probably be a bit more descriptive...
		match errno {
			libc::EOF => (io::EndOfFile, "end of file"),
			libc::ECONNREFUSED => (io::ConnectionRefused, "connection refused"),
			libc::ECONNRESET => (io::ConnectionReset, "connection reset"),
			libc::EPERM | libc::EACCES =>
				(io::PermissionDenied, "permission denied"),
			libc::EPIPE => (io::BrokenPipe, "broken pipe"),
			libc::ENOTCONN => (io::NotConnected, "not connected"),
			libc::ECONNABORTED => (io::ConnectionAborted, "connection aborted"),
			libc::EADDRNOTAVAIL => (io::ConnectionRefused, "address not available"),
			libc::EADDRINUSE => (io::ConnectionRefused, "address in use"),

			// These two constants can have the same value on some systems, but
			// different values on others, so we can't use a match clause
			x if x == libc::EAGAIN || x == libc::EWOULDBLOCK =>
				(io::ResourceUnavailable, "resource temporarily unavailable"),

			x => {
				println!("errno = {0}", errno);
				debug!("ignoring {}: {}", x, os::last_os_error());
				(io::OtherIoError, "unknown error")
			}
		}
	}

	let (kind, desc) = get_err(errno);
	IoError {
		kind: kind,
		desc: desc,
		detail: if detail {Some(os::last_os_error())} else {None},
	}
}

pub fn last_error() -> IoError { translate_error(os::errno() as i32, true) }