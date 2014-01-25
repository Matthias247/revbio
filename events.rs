// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io::IoError;
use std::rc::Rc;

pub enum EventKind
{
	StreamClosedEvent,
	IoErrorEvent(IoError),
	DataAvailableEvent(uint),
	TimerEvent,
	ChannelClosedEvent,
	ChannelMessageEvent,
	ConnectedEvent
}

pub struct Event
{
	event_type: EventKind,
	is_valid: bool,
	source: Rc<bool>
}

impl Event {
	pub fn originates_from<T:EventSource>(&self, source: &T) -> bool{
		return source.is_source_of(self);
	}
}

pub trait EventSource
{
	fn is_source_of(&self, event: &Event) -> bool;
}