#[no_uv];
extern mod native;
extern mod revbio;
extern mod extra;

use extra::time;

use revbio::events;
use revbio::channel::{Channel,Transmitter,BlockingReceiver,Receiver};
use revbio::{EventQueue};

#[start]
fn start(argc: int, argv: **u8) -> int {
	do native::start(argc, argv) {
		bench_conditions_main();
	}
}

fn timediff(start: time::Timespec, end: time::Timespec) -> f64 {
	let mut diff:f64;	

	if start.sec == end.sec {
		diff = ((end.nsec - start.nsec) as f64) / 1000000000f64;
	}
	else {
		diff = ((1000000000i32 - start.nsec) as f64) / 1000000000f64;
		let startsec = start.sec + 1;
		diff += ((end.sec - startsec) as f64);
		diff += ((end.nsec) as f64) / 1000000000f64;
	}
	diff
}


fn bench_conditions_main() {
	let ITERATIONS = 100000u32;

	let (port,chan): (Port<i32>, Chan<i32>) = Chan::new();
	let (rport,rchan): (Port<i32>, Chan<i32>) = Chan::new();
	let start_time = time::get_time();

	do native::task::spawn() {
		for _ in range(0,ITERATIONS) {
			rport.recv();
			chan.send(0);
		}
	}

	for _ in range(0,ITERATIONS) {
		rchan.send(0);
		port.recv();
	}

	let end_time = time::get_time();
	let diff = timediff(start_time, end_time);
	println!("Native channels: Diff: {:?}", diff);


	let (rx,tx): (BlockingReceiver<i32>, Transmitter<i32>) = Channel::create_blocking();
	let (rrx,rtx): (BlockingReceiver<i32>, Transmitter<i32>) = Channel::create_blocking();

	let start_time = time::get_time();

	do native::task::spawn() {
		for _ in range(0,ITERATIONS) {
			rrx.recv();
			tx.send(0);
		}
	}

	for _ in range(0,ITERATIONS) {
		rtx.send(0);
		rx.recv();
	}

	let end_time = time::get_time();
	let diff = timediff(start_time, end_time);
	println!("Revio blocking channels: Diff: {:?}", diff);


	let mut ev_queue = EventQueue::new();	
	let (mut rx,tx): (~Receiver<i32>, Transmitter<i32>) = Channel::create(&ev_queue);
	let (rrx,rtx): (BlockingReceiver<i32>, Transmitter<i32>) = Channel::create_blocking();
	
	let mut nr_received = 0u32;
	let start_time = time::get_time();

	do native::task::spawn() {
		let mut rev = EventQueue::new();
		let mut rselport = Receiver::from_blocking_receiver(rrx, &rev);
		for _ in range(0,ITERATIONS) {
			rev.next_event().unwrap();
			rselport.recv();
			tx.send(0);
		}
	}

	loop {
		rtx.send(0);
		let event = ev_queue.next_event().unwrap();			
		
		if event.originates_from(rx) {
			match event.event_type {
				events::ChannelMessageEvent => {
					rx.recv();
					nr_received += 1;
					if nr_received == ITERATIONS {
						break;
					}
				},
				_ => ()
			}
		}
	}	

	let end_time = time::get_time();
	let diff = timediff(start_time, end_time);
	println!("Revio event based channels: Diff: {:?}", diff);
}