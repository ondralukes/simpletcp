use simpletcp::simpletcp::{Message, TcpServer, TcpStream};
use std::thread::{sleep, spawn};
use std::time::Duration;

fn main() {
    let server = TcpServer::new("127.0.0.1:4234").unwrap();
    spawn(|| {
        //Give server some time to start
        sleep(Duration::from_millis(50));
        client_thread();
    });

    server_thread(server);
}

fn server_thread(server: TcpServer) {
    let mut client = server.accept_blocking().unwrap();

    println!("[Server] Accepted new client");
    client.wait_until_ready().unwrap();
    let mut i = 1;
    while i <= 3 {
        println!("[Server] Sending message [{}/3]", i);
        let mut msg = Message::new();
        msg.write_f64(1.23455);
        msg.write_buffer(&[3, 1, 4, 56]);
        client.write(&msg).unwrap();
        sleep(Duration::from_secs(1));
        i += 1;
    }
}

fn client_thread() {
    let mut client = TcpStream::connect("127.0.0.1:4234").unwrap();
    client.wait_until_ready().unwrap();

    loop {
        match client.read() {
            Ok(opt) => match opt {
                None => {}
                Some(mut msg) => {
                    println!(
                        "[Client] Received f64: {} and buffer: {:?}",
                        msg.read_f64().unwrap(),
                        msg.read_buffer().unwrap()
                    );
                }
            },
            Err(err) => {
                println!("[Client] {:?}", err);
                break;
            }
        }
    }
}
