#[no_uv];
extern mod native;
extern mod revbio;
extern mod extra;

use std::io::net::ip::{IpAddr,SocketAddr};

use revbio::events;
use revbio::channel::{Transmitter,BlockingReceiver,Channel};
use revbio::{EventQueue};
use revbio::tcp::{TcpSocket,RawTcpSocket,TcpServerSocket};

#[start]
fn start(argc: int, argv: **u8) -> int {
	native::start(argc, argv, proc() {
		main();
	})
}

fn main() {
	let mut ev_queue = EventQueue::new();
	let (rx,tx): (BlockingReceiver<bool>,Transmitter<bool>) = Channel::create_blocking();

	native::task::spawn(proc() {
		servertask(tx);
	});

	// Wait for the server to start up
	rx.recv();

	let opt_ipaddr:Option<IpAddr> = FromStr::from_str("127.0.0.1");
	let socketaddr = SocketAddr {ip: opt_ipaddr.unwrap(), port: 7000};
	let mut socket = TcpSocket::connect(socketaddr, &ev_queue).unwrap();
	let mut received_data = false;

	loop {
		let event = ev_queue.next_event().unwrap();			
		
		if event.originates_from(socket) {
			match event.event_type {
				events::ConnectedEvent => {
					println!("TCP stream got connected");
				},
				events::IoErrorEvent(err) => {
					println!("IoError: {}", err.desc.to_owned());
					if received_data { // Reconnect
						socket = TcpSocket::connect(socketaddr, &ev_queue).unwrap();
						received_data = false;
					}
					else {
						break;
					}
				},
				events::StreamClosedEvent => {
					println!("TCP connection closed");
					if received_data { // Reconnect
						socket = TcpSocket::connect(socketaddr, &ev_queue).unwrap();
						received_data = false;
					}
					else {
						break;
					}
				},
				events::DataAvailableEvent(nr_bytes) => {
					let mut buffer: ~[u8] = std::vec::from_elem::<u8>(nr_bytes, 0);
					let read_res = socket.read(buffer);
					match read_res {
						Err(err) => {
							println!("{}", err.desc.to_owned());
						}
						Ok(nr_read) => {
							received_data = true;
							let txt = std::str::from_utf8(buffer.slice(0, nr_read));
							println!("{}", txt);
						}
					}
				},
				_ => ()
			}
		}
	}
}

fn servertask(start_tx: Transmitter<bool>) {
	let mut client_count = 5; // Nr of clients to accept
	let mut ev_queue = EventQueue::new();

	let host = ~"0.0.0.0";
	let opt_ipaddr:Option<IpAddr> = FromStr::from_str(host);
	let socketaddr = SocketAddr{ip: opt_ipaddr.unwrap(), port: 7000};
	let opt_socket = TcpServerSocket::bind(socketaddr, 0, &ev_queue);
	if opt_socket.is_err() {
		println!("Error: {:?}", opt_socket.unwrap_err());
		fail!();
	}
	let mut server_socket = opt_socket.unwrap();

	start_tx.send(true);

	loop {
		let event = ev_queue.next_event().unwrap();			
		
		if event.originates_from(server_socket) {
			match event.event_type {
				events::ClientConnectedEvent => {
					println!("Client available");
					let acceptres = server_socket.accept();
					let socket = acceptres.unwrap();
					println!("Accepted a client");
					native::task::spawn(proc() {
						clienttask(socket);
					});
					client_count -= 1;
					if client_count == 0 {
						server_socket.close();
						break;
					}
				},
				events::IoErrorEvent(err) => {
					println!("IoError: {}", err.desc.to_owned());
					break;
				},
				_ => ()
			}
		}
		
	}	
}

fn clienttask(mut socket: RawTcpSocket) {
	let response = "HTTP 400 bad request";
	let _ = socket.write(response.as_bytes());
}