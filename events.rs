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
	ConnectedEvent,
	ClientConnectedEvent
}

pub struct Event
{
	event_type: EventKind,
	is_valid: bool,
	source: EventSourceId
}

impl Event {
	pub fn originates_from<T:EventSource>(&self, source: &T) -> bool{
		return source.is_source_of(self);
	}
}

/**
 * A structure that uniquely identifies the source of an event
 */
pub struct EventSourceId {
	priv id: Rc<bool>
}

impl EventSourceId {
	pub fn new() -> EventSourceId {
		EventSourceId {
			id: Rc::new(true)
		}
	}
}

impl Eq for EventSourceId {
	fn eq(&self, other: &EventSourceId) -> bool {
		if self.id.borrow() as *bool == other.id.borrow() as *bool {true}
		else {false}
	}
}

impl Clone for EventSourceId {
	fn clone(&self) -> EventSourceId {
		EventSourceId {
			id: self.id.clone()
		}
	}
}

/**
 * A trait that is implemented by possible sources of events
 */
pub trait EventSource
{
	fn is_source_of(&self, event: &Event) -> bool;
}