use std::{net::UdpSocket, thread, time::Duration};

fn main() {
    let broadcast_addr = "192.168.1.255:8080";

    // Listener
    let listener = UdpSocket::bind("0.0.0.0:8080").expect("could not bind listener");
    listener.set_broadcast(true).unwrap();

    thread::spawn(move || {
        let mut buf = [0; 1024];
        loop {
            if let Ok((size, addr)) = listener.recv_from(&mut buf) {
                println!(
                    "Received: {} from {}",
                    String::from_utf8_lossy(&buf[..size]),
                    addr
                );
            }
        }
    });

    // Sender
    let sender = UdpSocket::bind("0.0.0.0:0").expect("Could not bind sender");
    sender.set_broadcast(true).unwrap();

    loop {
        sender
            .send_to(b"Broadcast Test Data", &broadcast_addr)
            .unwrap();

        println!("Broadcasted event!");
        thread::sleep(Duration::from_secs(10));
    }
}
