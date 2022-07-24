







use core::time;
use std::io::Error;

use std::sync::{Arc, Mutex};
use std::thread;

use smoltcp::wire::{PrettyPrinter, EthernetFrame};

use std::os::unix::io::AsRawFd;
use tokio::io::{AsyncReadExt, ReadHalf, WriteHalf};
use tokio_tun::result::Result;
use tokio_tun::Tun;





const ARP: &[u8] = &[255, 255, 255, 255, 255, 255, 2, 0, 0, 0, 0, 2, 8, 6, 0, 1, 8, 0, 6, 4, 0, 1, 2, 0, 0, 0, 0, 2, 192, 168, 69, 1, 255, 255, 255, 255, 255, 255, 192, 168, 69, 100];
const SYN: &[u8] = &[138, 244, 12, 41, 208, 162, 2, 0, 0, 0, 0, 2, 8, 0, 69, 0, 0, 52, 0, 0, 64, 0, 64, 6, 221, 112, 192, 168, 69, 1, 104, 196, 238, 229, 244, 55, 0, 80, 97, 48, 190, 195, 0, 0, 0, 0, 128, 2, 4, 0, 253, 71, 0, 0, 2, 4, 5, 180, 3, 3, 0, 4, 2, 0, 0, 0];
const STN2: &[u8] = &[138, 244, 12, 41, 208, 162, 2, 0, 0, 0, 0, 2, 8, 0, 69, 0, 0, 40, 0, 0, 64, 0, 64, 6, 221, 124, 192, 168, 69, 1, 104, 196, 238, 229, 244, 55, 0, 80, 97, 48, 190, 196, 220, 230, 242, 180, 80, 16, 4, 0, 106, 104, 0, 0];

#[tokio::main]
async fn main() -> Result<()> {
    let tun = tokio_tun::TunBuilder::new()
        .name("tap0")
        .tap(true)
        .packet_info(false)
        .up()
        .try_build()?;

    println!("tun created, name: {}, fd: {}", tun.name(), tun.as_raw_fd());
    let (mut reader, mut _writer) = tokio::io::split(tun);
    listener(&mut reader).await;
    println!("a");
    listener(&mut reader).await;
    println!("b");

    let mut buf = [0u8; 1024];
    loop {
        let n = reader.read(&mut buf).await?;
        println!("reading {} bytes: {:?}", n, &buf[..n]);
    }
}
type Reader = Arc<Mutex<ReadHalf<Tun>>>;
type Writer = Arc<Mutex<WriteHalf<Tun>>>;

async fn listener(reader: &mut ReadHalf<Tun>){
    println!("hello");
    println!("hello");
}

fn notmain() {
    let iface = tun_tap::Iface::without_packet_info("tap0", tun_tap::Mode::Tap).expect("Failed Creat tap interface!");
    let iface = Arc::new(iface);
    let liface = Arc::clone(&iface);
    let wiface = Arc::clone(&iface);
    let listener = thread::spawn(move || {
        loop {
            let mut rs = [0;1054];
            let nbytes = liface.recv(&mut rs[..]).unwrap();
            println!(
                "{}",
                PrettyPrinter::<EthernetFrame<&[u8]>>::new("", &&rs[..nbytes])
                );
        }
    });
    let writer = thread::spawn(move || {
        // thread::sleep(Duration::from_secs(1));
        let packet = ARP;
        wiface.send(packet).unwrap();
        // thread::sleep(Duration::from_secs(1));
        let packet = SYN;
        wiface.send(packet).unwrap();
        let packet = STN2;
        wiface.send(packet).unwrap();
    });
    listener.join().unwrap();
    writer.join().unwrap();
    println!("Done");
}

