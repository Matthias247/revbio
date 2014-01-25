// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cast;
use std::libc;
use std::io;
use std::mem;
use std::os;
use std::io::IoError;
use std::io::net::ip;
use std::io::net::ip::{IpAddr,SocketAddr};
use std::cell::RefCell;
use std::rc::Rc;
use std::unstable::intrinsics;
use std::util::NonCopyable;
use extra::container::Deque;

use super::IoResult;
use super::events;
use super::eventqueue::EventQueue;
use super::eventqueueimpl::EventQueueImpl;
use super::syscalls;
use super::helpers;

fn htons(u: u16) -> u16 {
	intrinsics::to_be16(u as i16) as u16
}
// fn ntohs(u: u16) -> u16 {
// 	intrinsics::from_be16(u as i16) as u16
// }

enum InAddr {
	InAddr(libc::in_addr),
	In6Addr(libc::in6_addr),
}

fn ip_to_inaddr(ip: ip::IpAddr) -> InAddr {
	match ip {
		ip::Ipv4Addr(a, b, c, d) => {
			InAddr(libc::in_addr {
				s_addr: (d as u32 << 24) |
						(c as u32 << 16) |
						(b as u32 <<  8) |
						(a as u32 <<  0)
			})
		}
		ip::Ipv6Addr(a, b, c, d, e, f, g, h) => {
			In6Addr(libc::in6_addr {
				s6_addr: [
					htons(a),
					htons(b),
					htons(c),
					htons(d),
					htons(e),
					htons(f),
					htons(g),
					htons(h),
				]
			})
		}
	}
}

fn create_socket(addr: ip::SocketAddr, blocking: bool) -> IoResult<i32> {
	unsafe {
		let fam = match addr.ip {
			ip::Ipv4Addr(..) => libc::AF_INET,
			ip::Ipv6Addr(..) => libc::AF_INET6,
		};
		let mut ty: libc::c_int = libc::SOCK_STREAM | syscalls::SOCK_CLOEXEC;
		if !blocking {		
			ty |= syscalls::SOCK_NONBLOCK;
		};
		match libc::socket(fam, ty, 0) {
			-1 => Err(helpers::last_error()),
			fd => Ok(fd),
		}
	}
}

fn addr_to_sockaddr(addr: ip::SocketAddr) -> (libc::sockaddr_storage, uint) {
	unsafe {
		let storage: libc::sockaddr_storage = intrinsics::init();
		let len = match ip_to_inaddr(addr.ip) {
			InAddr(inaddr) => {
				let storage: *mut libc::sockaddr_in = cast::transmute(&storage);
				(*storage).sin_family = libc::AF_INET as libc::sa_family_t;
				(*storage).sin_port = htons(addr.port);
				(*storage).sin_addr = inaddr;
				mem::size_of::<libc::sockaddr_in>()
			}
			In6Addr(inaddr) => {
				let storage: *mut libc::sockaddr_in6 = cast::transmute(&storage);
				(*storage).sin6_family = libc::AF_INET6 as libc::sa_family_t;
				(*storage).sin6_port = htons(addr.port);
				(*storage).sin6_addr = inaddr;
				mem::size_of::<libc::sockaddr_in6>()
			}
		};
		return (storage, len);
	}
}

#[deriving(Eq)]
pub enum ConnectionState {
	Created = 0,
	Connecting = 1,
	Connected = 2,
	Closed = 3
}

pub struct RawTcpSocket {
	priv fd: i32,
	priv blocking: bool,
	priv connection_state: ConnectionState,
	priv nc: NonCopyable
}

impl RawTcpSocket {
	pub fn connect(addr: ip::SocketAddr) -> IoResult<RawTcpSocket> {
		unsafe {
			create_socket(addr, true).and_then(|fd| {
				let (addr, len) = addr_to_sockaddr(addr);
				let addrp = &addr as *libc::sockaddr_storage;
				let ret = RawTcpSocket::from_fd(fd);
				match helpers::retry(|| {
					libc::connect(fd, addrp as *libc::sockaddr,
								  len as libc::socklen_t)
				}) {
					-1 => Err(helpers::last_error()),
					_ => Ok(ret),
				}
			})
		}
	}

	fn from_fd(fd: i32) -> RawTcpSocket {
		RawTcpSocket { 
			fd: fd, 
			blocking: true,
			connection_state: Connected,
			nc: NonCopyable
		}
	}

	fn set_blocking(&mut self, blocking: bool) {
		if self.connection_state == Closed { return; }
		if self.blocking == blocking { return; }
		syscalls::set_fd_blocking(self.fd, blocking);
		self.blocking = blocking;
	}

	fn close_socket(&mut self) {
		if self.connection_state == Closed { return; }
		self.connection_state = Closed;
		unsafe { libc::close(self.fd); }
	}

	pub fn close(&mut self) {
		self.close_socket();
	}

	pub fn write(&mut self, buf: &[u8]) -> IoResult<(uint)> {
		if self.connection_state != Connected {
			return Err(IoError{
				kind: io::Closed,
				desc: "Connection is closed",
				detail: None
			});
		}
		let len = buf.len();
		let data = buf.as_ptr();
		if len == 0 {
			return Ok(0);
		}

		let ret:libc::c_int = helpers::retry(|| {
			unsafe {
				libc::send(self.fd,
					data as *mut libc::c_void,
					len as libc::size_t,
					0) as libc::c_int
			}
		});

		if ret < 0 {
			let errno = os::errno() as int;
			if (errno != libc::EWOULDBLOCK as int && errno != libc:: EAGAIN as int) {
				self.close_socket();
			}
			Err(helpers::last_error())
		} else {
			Ok(ret as uint)
		}
	}

	pub fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
		if self.connection_state != Connected {
			return Err(IoError{
				kind: io::Closed,
				desc: "Connection is closed",
				detail: None
			});
		}
		if buf.len() == 0 {
			return Ok(0);
		}
		let ret = helpers::retry(|| {
			unsafe {
				libc::recv(self.fd,
						   buf.as_ptr() as *mut libc::c_void,
						   buf.len() as libc::size_t,
						   0) as libc::c_int
			}
		});
		if ret == 0 { // TODO: Check if that's correct for nonblocking IO
			self.close_socket();
			Err(io::standard_error(io::EndOfFile))
		} 
		else if ret < 0 {
			let errno = os::errno() as int;
			if (errno != libc::EWOULDBLOCK as int && errno != libc:: EAGAIN as int) {
				self.close_socket();
			}
			Err(helpers::last_error())
		} else {
			Ok(ret as uint)
		}
	}
}

impl Drop for RawTcpSocket {
	fn drop(&mut self) {
		self.close_socket();
	}
}

pub struct TcpSocket {
	priv process_func: fn(func_ptr: *libc::c_void, event_queue: &mut EventQueueImpl, epoll_events: u32),
	priv socket: RawTcpSocket,
	priv event_queue: Rc<RefCell<EventQueueImpl>>,
	priv event_source_handle: Rc<bool>,
	priv epoll_events: u32,
	priv available_bytes: uint
}

impl events::EventSource for TcpSocket {
	fn is_source_of(&self, event: &events::Event) -> bool {
		if self.event_source_handle.borrow() as *bool == event.source.borrow() as *bool {true}
		else { false }
	}
}

impl TcpSocket {

	pub fn from_raw_tcp_socket(raw_tcp_socket: RawTcpSocket, event_queue: &EventQueue) -> ~TcpSocket {
		let mut socket = ~TcpSocket {
			socket: raw_tcp_socket,
			event_queue: event_queue._get_impl(),
			process_func: TcpSocket::process_epoll_events,
			event_source_handle: Rc::new(true),
			epoll_events: 0,
			available_bytes: 0
		};
		if socket.socket.connection_state != Closed {
			socket.register_fd();
		}
		socket
	}

	pub fn connect(addr: ip::SocketAddr, event_queue: &EventQueue) -> IoResult<~TcpSocket> {
		unsafe {
			create_socket(addr, false).and_then(|fd| {
				let (addr, len) = addr_to_sockaddr(addr);
				let addrp = &addr as *libc::sockaddr_storage;
				let mut rawsock = RawTcpSocket { 
					fd: fd, 
					blocking: false,
					connection_state: Created,
					nc: NonCopyable };
				match helpers::retry(|| {
					libc::connect(fd, addrp as *libc::sockaddr,
								  len as libc::socklen_t)
				}) {
					-1 => {
						let errno = os::errno() as i32;
						if errno != libc::EINPROGRESS {
							println!("Failed on connect");
							Err(helpers::last_error())
						}
						else {
							rawsock.connection_state = Connecting;
							let ret = TcpSocket::from_raw_tcp_socket(rawsock, event_queue);
							Ok(ret)
						}
					},
					_ => { // Strangely we were connected synchronously
						println!("Direct connect");
						rawsock.connection_state = Connected;
						rawsock.set_blocking(true);
						let ret = TcpSocket::from_raw_tcp_socket(rawsock, event_queue);
						ret.event_queue.borrow().with_mut(|eq| {
							let evt = events::Event {
								event_type: events::ConnectedEvent,
								is_valid: true,
								source: ret.event_source_handle.clone()
							};
							eq.ready_events.push_back(evt); 
						});
						Ok(ret)
					}
				}
			})
		}
	}

	pub fn close(&mut self) {
		self.remove_pending_events();
		self.socket.close_socket();
	}

	pub fn write(&mut self, buf: &[u8]) -> IoResult<(uint)> {
		let state = self.socket.connection_state;
		let ret = self.socket.write(buf);
		// Check if the the write caused an error/close
		if state != self.socket.connection_state
		   && self.socket.connection_state == Closed {
			self.remove_pending_events();
			// TODO: Should an unsuccessful write queue an error event?
			// And should it really kill possible data to read?
		}
		ret
	}

	pub fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
		let state = self.socket.connection_state;
		if (self.socket.connection_state == Connected 
			&& self.available_bytes == 0) {
			Ok(0)
		}
		else {
			let ret = self.socket.read(buf);
			match ret {
				Ok(read_bytes) => { // Downcount available bytes
					self.available_bytes -= read_bytes;
					Ok(read_bytes)
				}
				Err(err) => {
					if state != self.socket.connection_state
					   && self.socket.connection_state == Closed {
						self.remove_pending_events();
					}
					Err(err)
				}
			}
		}
	}

	/*fn set_blocking(&mut self, blocking: bool) {
		// TODO: Allow during active operation?
		self.socket.set_blocking(blocking)
	}*/

	fn read_available_bytes(&mut self) -> IoResult<uint> {
		let bytes_available:i32 = 0;
		let ret = helpers::retry(|| unsafe {
			syscalls::ioctl(self.socket.fd, syscalls::FIONREAD, &bytes_available)
		});
		if ret < 0 {
			self.available_bytes = 0;
			Err(helpers::last_error())
		} else {
			self.available_bytes = bytes_available as uint;
			Ok(bytes_available as uint)
		}
	}

	/**
	 * Registers the fd for reading if it's connected or for writing on connects
	 */
	fn register_fd(&mut self) {
		let callback: *libc::c_void = unsafe { cast::transmute(&self.process_func) };
		self.event_queue.borrow().with_mut(|q| {
			if self.socket.connection_state == Connected {
				q.register_fd(self.socket.fd, syscalls::EPOLLIN, callback)
			}
			else {
				q.register_fd(self.socket.fd, syscalls::EPOLLOUT, callback)
			}
		});
	}

	fn remove_pending_events(&mut self) {
		self.event_queue.borrow().with_mut(|q|
			q.remove_pending_events(
				|ev|ev.source == self.event_source_handle)
		);
	}

	fn process_epoll_events(func_ptr: *libc::c_void, event_queue: &mut EventQueueImpl, epoll_events: u32) {
		unsafe {
			let sock: *mut TcpSocket = func_ptr as *mut TcpSocket;

			if (epoll_events & syscalls::EPOLLERR != 0) { // Read the error
				let mut resBuffer = [0, ..0]; // TODO: This gives false positives
				let res = (*sock).socket.read(resBuffer);
				match res {
					Ok(_) => fail!("There seems to be a logic in error in this socket implementation"),
					Err(err) => {
						let e = events::Event {
							event_type: events::IoErrorEvent(err),
							is_valid: true,
							source: (*sock).event_source_handle.clone()
						};
						(*sock).socket.close_socket();
						// There is no need to remove pending events
						// because if there would be any this function
						// wouldn't have been called
						event_queue.ready_events.push_back(e);
					}
				}
			}
			else {
				if (*sock).socket.connection_state == Connected {
					if ((epoll_events & syscalls::EPOLLIN != 0) || (epoll_events & syscalls::EPOLLERR != 0)) {
						if (epoll_events & syscalls::EPOLLHUP == 0) {
							(*sock).read_available_bytes();
						} else {
							(*sock).available_bytes = 0;
						}
						if (*sock).available_bytes > 0 {
							let e = events::Event {
								event_type: events::DataAvailableEvent((*sock).available_bytes),
								is_valid: true,
								source: (*sock).event_source_handle.clone()
							};
							event_queue.ready_events.push_back(e);
						} else {
							let e = events::Event {
								event_type: events::StreamClosedEvent,
								is_valid: true,
								source: (*sock).event_source_handle.clone()
							};
							(*sock).socket.close_socket();
							event_queue.ready_events.push_back(e);
						}
					}
					// Currently not used
					/*if (epoll_events & syscalls::EPOLLOUT != 0) {
					}*/
				}
				else { // Connecting
					if (epoll_events & syscalls::EPOLLOUT != 0) {
						
						let out: libc::c_int = 0;
						let outlen: libc::socklen_t = mem::size_of::<libc::c_int>() as libc::socklen_t;
						let ret = syscalls::getsockopt(
							(*sock).socket.fd, libc::SOL_SOCKET, syscalls::SO_ERROR,
							&out as *libc::c_int as *libc::c_void,
							&outlen as *libc::socklen_t);

						// Read and evaluate if connect was successful
						let mut success = ret != -1;
						let mut errno;						
						if success {
							success = out == 0;
							errno = out;
						}
						else {
							errno = os::errno() as libc::c_int;
						}
						
						if !success {
							let err = helpers::translate_error(errno, false);
							let e = events::Event {
								event_type: events::IoErrorEvent(err),
								is_valid: true,
								source: (*sock).event_source_handle.clone()
							};
							(*sock).socket.close_socket();
							event_queue.ready_events.push_back(e);
						}
						else { // Connect was successful
							(*sock).socket.set_blocking(true);
							(*sock).socket.connection_state = Connected;
							let e = events::Event {
								event_type: events::ConnectedEvent,
								is_valid: true,
								source: (*sock).event_source_handle.clone()
							};							
							let callback: *libc::c_void = cast::transmute(&(*sock).process_func);
							// Switch interest to EPOLLIN
							event_queue.modify_fd((*sock).socket.fd, syscalls::EPOLLIN, callback);							
							event_queue.ready_events.push_back(e);
						}
					}
				}
			}
		}
	}
}

#[unsafe_destructor]
impl Drop for TcpSocket {
	fn drop(&mut self) {
		self.remove_pending_events();
	}
}

pub struct RawTcpServerSocket {
	priv fd: i32,
	priv connection_state: ConnectionState,
}

impl RawTcpServerSocket {
	// TODO: Ipv6-only parameter
	pub fn bind(addr: ip::SocketAddr, backlog: i32) -> IoResult<RawTcpServerSocket> {
		unsafe {
			create_socket(addr, true).and_then(|fd| {
				let (addr, len) = addr_to_sockaddr(addr);
				let addrp = &addr as *libc::sockaddr_storage;
				let ret = RawTcpServerSocket { 
					fd: fd,
					connection_state: Connected,
					};
				match libc::bind(fd, addrp as *libc::sockaddr,
								 len as libc::socklen_t) {
					-1 => Err(helpers::last_error()),
					_ => {
						match libc::listen(fd, backlog) {
							-1 => Err(helpers::last_error()),
							_ => Ok(ret)
						}
					}
				}
			})
		}
	}

	pub fn accept(&mut self) -> IoResult<RawTcpSocket> {
		if self.connection_state != Connected {
			return Err(IoError{
				kind: io::Closed,
				desc: "Socket is closed",
				detail: None
			});
		}
		unsafe {
			let mut storage: libc::sockaddr_storage = intrinsics::init();
			let storagep = &mut storage as *mut libc::sockaddr_storage;
			let size = mem::size_of::<libc::sockaddr_storage>();
			let mut size = size as libc::socklen_t;
			match helpers::retry(|| {
				libc::accept(self.fd,
							 storagep as *mut libc::sockaddr,
							 &mut size as *mut libc::socklen_t) as libc::c_int
			}) {
				-1 => {
					let errno = os::errno() as int;
					if (errno != libc::EWOULDBLOCK as int && errno != libc::EAGAIN as int) {
						self.close_socket();
					}
					Err(helpers::last_error())
				},
				fd => Ok(RawTcpSocket::from_fd(fd))
			}
		}
	}

	fn close_socket(&mut self) {
		if self.connection_state == Closed { return; }
		self.connection_state = Closed;
		unsafe { libc::close(self.fd); }
	}

	pub fn close(&mut self) {
		self.close_socket();
	}
}

impl Drop for RawTcpServerSocket {
	fn drop(&mut self) {
		self.close_socket();
	}
}

pub struct TcpServerSocket {
	priv process_func: fn(func_ptr: *libc::c_void, event_queue: &mut EventQueueImpl, epoll_events: u32),
	priv socket: RawTcpServerSocket,
	priv event_queue: Rc<RefCell<EventQueueImpl>>,
	priv event_source_handle: Rc<bool>,
	priv epoll_events: u32,
	priv client_available: bool
}

impl events::EventSource for TcpServerSocket {
	fn is_source_of(&self, event: &events::Event) -> bool {
		if self.event_source_handle.borrow() as *bool == event.source.borrow() as *bool {true}
		else { false }
	}
}

impl TcpServerSocket {

	pub fn from_raw_server_socket(raw_server_socket: RawTcpServerSocket, event_queue: &EventQueue) -> ~TcpServerSocket {
		let mut socket = ~TcpServerSocket {
			socket: raw_server_socket,
			event_queue: event_queue._get_impl(),
			process_func: TcpServerSocket::process_epoll_events,
			event_source_handle: Rc::new(true),
			epoll_events: 0,
			client_available: false
		};
		if socket.socket.connection_state != Closed {
			socket.register_fd();
		}
		socket
	}

	pub fn bind(addr: ip::SocketAddr, backlog: i32, event_queue: &EventQueue) -> IoResult<~TcpServerSocket> {
		let sock = RawTcpServerSocket::bind(addr, backlog);
		match sock {
			Ok(sock) => {
				Ok(TcpServerSocket::from_raw_server_socket(sock, event_queue))
			},
			Err(err) => Err(err)
		}
	}

	pub fn close(&mut self) {
		self.socket.close_socket();
		self.remove_pending_events();
	}

	pub fn accept(&mut self) -> IoResult<RawTcpSocket> {
		let state = self.socket.connection_state;
		if (self.socket.connection_state == Connected 
			&& !self.client_available) { // Currently no client available
			Err(IoError{
				kind: io::ResourceUnavailable,
				desc: "No client available",
				detail: None
			})
		}
		else {
			let ret = self.socket.accept();
			match ret {
				Ok(socket) => {
					self.client_available = false;
					Ok(socket)
				}
				Err(err) => {
					if state != self.socket.connection_state
					   && self.socket.connection_state == Closed {
						self.remove_pending_events();
					}
					Err(err)
				}
			}
		}
	}

	/**
	 * Registers the fd for reading if it's connected or for writing on connects
	 */
	fn register_fd(&mut self) {
		let callback: *libc::c_void = unsafe { cast::transmute(&self.process_func) };
		self.event_queue.borrow().with_mut(|q| {
			q.register_fd(self.socket.fd, syscalls::EPOLLIN, callback)
		});
	}

	fn remove_pending_events(&mut self) {
		self.event_queue.borrow().with_mut(|q|
			q.remove_pending_events(
				|ev|ev.source == self.event_source_handle)
		);
	}

	fn process_epoll_events(func_ptr: *libc::c_void, event_queue: &mut EventQueueImpl, epoll_events: u32) {
		unsafe {
			let sock: *mut TcpServerSocket = func_ptr as *mut TcpServerSocket;

			if (epoll_events & syscalls::EPOLLERR != 0) { // Read the error
				let res = (*sock).socket.accept();
				match res {
					Ok(_) => fail!("There seems to be a logic in error in this socket implementation"),
					Err(err) => {
						let e = events::Event {
							event_type: events::IoErrorEvent(err),
							is_valid: true,
							source: (*sock).event_source_handle.clone()
						};
						(*sock).socket.close_socket();
						// There is no need to remove pending events
						// because if there would be any this function
						// wouldn't have been called
						event_queue.ready_events.push_back(e);
					}
				}
			}
			else {
				if epoll_events & syscalls::EPOLLIN != 0 {
					(*sock).client_available = true;					
					let e = events::Event {
						event_type: events::ClientConnectedEvent,
						is_valid: true,
						source: (*sock).event_source_handle.clone()
					};
					event_queue.ready_events.push_back(e);
				}
			}
		}
	}
}

#[unsafe_destructor]
impl Drop for TcpServerSocket {
	fn drop(&mut self) {
		self.remove_pending_events();
	}
}
