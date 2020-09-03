use rand::prelude::StdRng;
use rand::{RngCore, SeedableRng};
use simpletcp::simpletcp::{Message, TcpStream};
use std::convert::TryInto;
use std::time::Instant;

fn main() {
    println!("Connecting");
    let mut conn = TcpStream::connect("127.0.0.1:4328").unwrap();
    conn.wait_until_ready().unwrap();
    println!("Connection ready");

    let mut rand = StdRng::from_entropy();
    loop {
        let mut rand_buffer = [8; 8];
        rand.fill_bytes(&mut rand_buffer[0..8]);

        rand_buffer[3] = 0;
        rand_buffer[7] = 0;

        let a = i32::from_le_bytes(rand_buffer[0..4].try_into().unwrap());
        let b = i32::from_le_bytes(rand_buffer[4..8].try_into().unwrap());
        print!("{:010} + {:010} = ", a, b);
        let mut message = Message::new();
        message.write_i32(a);
        message.write_i32(b);
        let time = Instant::now();
        conn.write(&message).unwrap();

        let mut response = conn.read_blocking().unwrap();
        println!(
            "{:010} [{} us]",
            response.read_i32().unwrap(),
            time.elapsed().as_micros()
        );
    }
}
