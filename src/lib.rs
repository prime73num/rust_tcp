use std::collections::VecDeque;
use std::error::Error;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::{thread, io};


use tun_tap;
use smoltcp::wire::{self, PrettyPrinter};






struct Quad {
    src: (Ipv4Addr, u16),
    dst: (Ipv4Addr, u16),
}

struct Connection {
    
}

struct ConnectionPool_inner {
}

impl Default for ConnectionPool_inner {
    fn default() -> Self {
        ConnectionPool_inner::new()
    }
}

impl ConnectionPool_inner {
    fn new() -> Self {
        Self {}
    }
    fn add()  {
        unimplemented!()
    }
    fn get() -> io::Result<Connection> {
        unimplemented!()
    }
    fn remove()  {
        unimplemented!()
    }
    fn bind(port: u16) {
        unimplemented!()
    }
}

type ConnectionPool = Arc<Mutex<ConnectionPool_inner>>;



struct Interface {
    connection_pool: ConnectionPool,
    tuntap_interface: Arc<tun_tap::Iface>
}

// Establish a connection Remove a connection Send messge to connection.
fn packet_loop(nic: Arc<tun_tap::Iface>, connection_pool: ConnectionPool) -> io::Result<()>{
    let mut buf = [0u8; 1504];
    let mut try_recv = move || -> Result<(), Box<dyn Error>> {
        let nbytes = nic.recv(&mut buf[..])?;
        println!(
            "{}",
            PrettyPrinter::<wire::Ipv4Packet<&[u8]>>::new("", &&buf[..nbytes])
            );
        let ipv4header = wire::Ipv4Packet::new_checked(&buf[..nbytes])?;
        let tcpheader = wire::TcpPacket::new_checked(ipv4header.payload())?;
        Ok(())
    };
    loop {
        if let Err(e) = try_recv() {
            println!("Skip because of {}", &e);
        }
    }
    Ok(())
}

impl Interface {
    fn new() -> io::Result<Self> {
        let nic = tun_tap::Iface::without_packet_info("tun0", tun_tap::Mode::Tun)?;
        let nic = Arc::new(nic);
        let nic_temp = nic.clone();
        let con_pool: ConnectionPool = Arc::default();
        let temp = con_pool.clone();
        thread::spawn(move || packet_loop( nic_temp, temp));
        Ok( Self {
            connection_pool: con_pool,
            tuntap_interface: nic.clone()
        })
    }
    fn bind() -> io::Result<TcpListener> {
        unimplemented!()
    }
    fn connect() -> io::Result<TcpStream> {
        unimplemented!()
    }
}

struct TcpListener {
    port: u16,
    con_pool: ConnectionPool
}

struct TcpStream {
    quad: Quad,
    connection: Connection
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;
    // #[test]
    fn test_packet_loop() {
        let nic = tun_tap::Iface::without_packet_info("tap0", tun_tap::Mode::Tap).expect("Failed Creat nic");
        let nic = Arc::new(nic);
        let con_pool: ConnectionPool = Arc::default();
        packet_loop(nic, con_pool);
    } 
    #[test]
    fn test_interface() {
        let ih = Interface::new().unwrap();
        let  arp_packet: &[u8] = &[255, 255, 255, 255, 255, 255, 2, 0, 0, 0, 0, 2, 8, 6, 0, 1, 8, 0, 6, 4, 0, 1, 2, 0, 0, 0, 0, 2, 192, 168, 69, 1, 255, 255, 255, 255, 255, 255, 192, 168, 69, 100];
        let syn_packet: &[u8] = &[138, 244, 12, 41, 208, 162, 2, 0, 0, 0, 0, 2, 8, 0, 69, 0, 0, 52, 0, 0, 64, 0, 64, 6, 221, 112, 192, 168, 69, 1, 104, 196, 238, 229, 244, 55, 0, 80, 97, 48, 190, 195, 0, 0, 0, 0, 128, 2, 4, 0, 253, 71, 0, 0, 2, 4, 5, 180, 3, 3, 0, 4, 2, 0, 0, 0];
        let ack_packet: &[u8] = &[138, 244, 12, 41, 208, 162, 2, 0, 0, 0, 0, 2, 8, 0, 69, 0, 0, 40, 0, 0, 64, 0, 64, 6, 221, 124, 192, 168, 69, 1, 104, 196, 238, 229, 244, 55, 0, 80, 97, 48, 190, 196, 220, 230, 242, 180, 80, 16, 4, 0, 106, 104, 0, 0];
        println!("interface send");
        ih.tuntap_interface.send(&arp_packet[..]);
        thread::sleep(Duration::from_secs(10));
        ih.tuntap_interface.send(&syn_packet[..]);
        thread::sleep(Duration::from_secs(10));
        ih.tuntap_interface.send(&ack_packet[..]);
        thread::sleep(Duration::from_secs(10));
    }
}
