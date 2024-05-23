use anyhow::Result;
use bytes::{BufMut, BytesMut};
fn main() -> Result<()> {
    let mut buf = BytesMut::with_capacity(1024);
    buf.extend_from_slice(b"hello world\n");
    buf.put(&b"goodbye world"[..]);
    buf.put_i64(0xdeadbeef); // big endian put the data as we see it, it's same as network byte order
                             // buf.put_i64_le(0xdeadbeef);
    println!("{:?}", buf);
    let a = buf.split();
    let mut b = a.freeze(); // inner buffer is now immutable

    let pos = b.as_ref().iter().position(|&x| x == b'\n').unwrap();
    let c = b.split_to(pos + 1);
    println!("{:?}", c);
    println!("{:?}", b);
    println!("{:?}", buf);
    Ok(())
}
