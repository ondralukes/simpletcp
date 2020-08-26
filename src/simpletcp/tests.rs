use crate::simpletcp::{Message, TcpServer, TcpStream};
use std::io::Write;
use std::net;
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};

#[test]
fn create_server() {
    TcpServer::new("127.0.0.1:1234").expect("Failed to create server");
}

#[test]
fn accept_none() {
    let server = TcpServer::new("127.0.0.1:1235").expect("Failed to create server");
    match server.accept().expect("Accept failed") {
        None => {}
        Some(_) => {
            panic!("Accept returned Some but None was expected");
        }
    }
}

#[test]
fn connect() {
    let _server = TcpServer::new("127.0.0.1:1236").expect("Failed to create server");
    let _client = TcpStream::connect("127.0.0.1:1236").expect("Failed to connect to server");
}

#[test]
fn accept() {
    let server = TcpServer::new("127.0.0.1:1237").expect("Failed to create server");
    let _client = TcpStream::connect("127.0.0.1:1237").expect("Failed to connect to server");

    let time = Instant::now();
    loop {
        match server.accept().unwrap() {
            None => {}
            Some(_) => {
                break;
            }
        }
        if time.elapsed().as_millis() > 500 {
            panic!("Timeout");
        }
    }
}

#[test]
fn accept_blocking() {
    let server = TcpServer::new("127.0.0.1:1237").expect("Failed to create server");
    let _client = TcpStream::connect("127.0.0.1:1237").expect("Failed to connect to server");

    let _s_client = server.accept_blocking().unwrap();
}

#[test]
fn raw() {
    let server = TcpServer::new("127.0.0.1:1238").expect("Failed to create server");
    let mut client = TcpStream::connect("127.0.0.1:1238").expect("Failed to connect to server");

    let time = Instant::now();
    loop {
        match server.accept().unwrap() {
            None => {}
            Some(mut s_client) => {
                client.write_raw(&[1, 2, 3]).unwrap();

                loop {
                    match s_client.read_raw().unwrap() {
                        None => {}
                        Some(msg) => {
                            assert_eq!(msg, vec![1, 2, 3]);
                            break;
                        }
                    }
                    if time.elapsed().as_millis() > 500 {
                        panic!("Timeout");
                    }
                }
                break;
            }
        }
        if time.elapsed().as_millis() > 500 {
            panic!("Timeout");
        }
    }
}

#[test]
fn raw_would_block() {
    let _server = TcpServer::new("127.0.0.1:1239").expect("Failed to create server");
    let mut client = TcpStream::connect("127.0.0.1:1239").expect("Failed to connect to server");

    assert_eq!(client.read_raw().unwrap(), None);
}

#[test]
fn raw_fragmented() {
    let server = TcpServer::new("127.0.0.1:1240").expect("Failed to create server");
    let mut client =
        net::TcpStream::connect("127.0.0.1:1240").expect("Failed to connect to server");

    let time = Instant::now();
    loop {
        match server.accept().unwrap() {
            None => {}
            Some(mut s_client) => {
                client.write(&[3, 0]).unwrap();
                assert_eq!(s_client.read_raw().unwrap(), None);

                client.write(&[0]).unwrap();
                assert_eq!(s_client.read_raw().unwrap(), None);

                client.write(&[0, 1, 2]).unwrap();
                assert_eq!(s_client.read_raw().unwrap(), None);

                client.write(&[3, 1, 0]).unwrap();
                assert_read_raw(&mut s_client, &vec![1, 2, 3]);

                client.write(&[0, 0, 7]).unwrap();
                assert_read_raw(&mut s_client, &vec![7]);

                break;
            }
        }
        if time.elapsed().as_millis() > 500 {
            panic!("Timeout");
        }
    }
}

fn assert_read_raw(socket: &mut TcpStream, expected: &Vec<u8>) {
    let time = Instant::now();
    loop {
        match socket.read_raw().unwrap() {
            None => {}
            Some(res) => {
                assert_eq!(&res, expected);
                break;
            }
        }

        if time.elapsed().as_millis() > 500 {
            panic!("Timeout");
        }
    }
}

#[test]
fn client_write_server_read() {
    let server = TcpServer::new("127.0.0.1:1241").expect("Failed to create server");
    spawn(|| {
        let mut client = TcpStream::connect("127.0.0.1:1241").expect("Failed to connect to server");
        client.wait_until_ready().unwrap();
        let mut msg = Message::new();
        msg.write_i32(-123);
        msg.write_i8(-5);
        client.write(&msg).unwrap();
    });

    let time = Instant::now();
    loop {
        let s_client = server.accept().unwrap();
        match s_client {
            None => {}
            Some(mut s_client) => {
                s_client.wait_until_ready().unwrap();
                let mut msg = s_client.read().unwrap().unwrap();
                assert_eq!(msg.read_i32().unwrap(), -123);
                assert_eq!(msg.read_i8().unwrap(), -5);
                assert!(msg.read_i32().is_err());
                break;
            }
        }
        if time.elapsed().as_millis() > 500 {
            panic!("Timeout");
        }
    }
}

#[test]
fn server_write_client_read() {
    let server = TcpServer::new("127.0.0.1:1242").expect("Failed to create server");

    spawn(move || loop {
        let s_client = server.accept().unwrap();
        match s_client {
            None => {}
            Some(mut s_client) => {
                s_client.wait_until_ready().unwrap();
                let mut msg = Message::new();
                msg.write_i64(-12345);
                msg.write_u8(23);
                s_client.write(&msg).unwrap();
                break;
            }
        }
    });

    let mut client = TcpStream::connect("127.0.0.1:1242").expect("Failed to connect to server");
    client.wait_until_ready().unwrap();

    let time = Instant::now();
    loop {
        match client.read().unwrap() {
            None => {}
            Some(mut msg) => {
                assert_eq!(msg.read_i64().unwrap(), -12345);
                assert!(msg.read_i16().is_err());
                break;
            }
        }
        if time.elapsed().as_millis() > 500 {
            panic!("Timeout");
        }
    }
}

#[test]
fn read_timeout_timed_out() {
    let server = TcpServer::new("127.0.0.1:4242").expect("Failed to create server");

    spawn(move || loop {
        let s_client = server.accept().unwrap();
        match s_client {
            None => {}
            Some(mut s_client) => {
                s_client.wait_until_ready().unwrap();
                sleep(Duration::from_millis(1200));
                break;
            }
        }
    });

    let mut client = TcpStream::connect("127.0.0.1:4242").expect("Failed to connect to server");
    client.wait_until_ready().unwrap();
    let time = Instant::now();
    client.read_timeout(1000).unwrap();
    let time = time.elapsed().as_millis();
    assert!(time > 950);
}

#[test]
fn read_timeout_success() {
    let server = TcpServer::new("127.0.0.1:3242").expect("Failed to create server");

    spawn(move || loop {
        let s_client = server.accept().unwrap();
        match s_client {
            None => {}
            Some(mut s_client) => {
                s_client.wait_until_ready().unwrap();
                sleep(Duration::from_millis(500));
                let mut m = Message::new();
                m.write_u32(1);
                s_client.write(&m).unwrap();
                sleep(Duration::from_millis(500));
                break;
            }
        }
    });

    let mut client = TcpStream::connect("127.0.0.1:3242").expect("Failed to connect to server");
    client.wait_until_ready().unwrap();
    let time = Instant::now();
    let res = client.read_timeout(1000).unwrap();
    let time = time.elapsed().as_millis();
    assert!(time > 450 && time < 1000);
    assert_eq!(res.unwrap().read_u32().unwrap(), 1);
}

#[test]
fn buffer_overflow(){

    let server = TcpServer::new("127.0.0.1:1441").expect("Failed to create server");
    spawn(|| {
        let mut client = TcpStream::connect("127.0.0.1:1441").expect("Failed to connect to server");
        client.wait_until_ready().unwrap();
        let buf = [0; 65537];
        let mut msg = Message::new();
        msg.write_buffer(&buf);

        let mut i = 0;
        while i < 100{
            client.write(&msg).unwrap();
            i+=1;
        }


        while !client.flush().unwrap(){
        }
        sleep(Duration::from_secs(1));
    });

    let time = Instant::now();
    loop {
        let s_client = server.accept().unwrap();
        match s_client {
            None => {}
            Some(mut s_client) => {
                s_client.wait_until_ready().unwrap();
                let mut i = 0;
                while i < 100 {
                    let mut msg = s_client.read_blocking().unwrap();
                    assert_eq!(msg.read_buffer().unwrap(), vec![0; 65537]);
                    i += 1;
                }
                break;
            }
        }
        if time.elapsed().as_millis() > 500 {
            panic!("Timeout");
        }
    }
}

#[test]
fn message_types() {
    let mut m = Message::new();
    m.write_u8(1);
    m.write_i8(-1);
    m.write_u16(1);
    m.write_i16(-1);
    m.write_u32(1);
    m.write_i32(-1);
    m.write_u64(1);
    m.write_i64(-1);
    m.write_u128(1);
    m.write_i128(-1);
    m.write_f32(0.1);
    m.write_f64(f64::INFINITY);
    m.write_buffer(&[1, 2, 3, 4]);

    assert_eq!(m.read_u8().unwrap(), 1);
    assert_eq!(m.read_i8().unwrap(), -1);
    assert_eq!(m.read_u16().unwrap(), 1);
    assert_eq!(m.read_i16().unwrap(), -1);
    assert_eq!(m.read_u32().unwrap(), 1);
    assert_eq!(m.read_i32().unwrap(), -1);
    assert_eq!(m.read_u64().unwrap(), 1);
    assert_eq!(m.read_i64().unwrap(), -1);
    assert_eq!(m.read_u128().unwrap(), 1);
    assert_eq!(m.read_i128().unwrap(), -1);
    assert_eq!(m.read_f32().unwrap(), 0.1);
    assert_eq!(m.read_f64().unwrap(), f64::INFINITY);
    assert_eq!(m.read_buffer().unwrap(), vec![1, 2, 3, 4]);
}
